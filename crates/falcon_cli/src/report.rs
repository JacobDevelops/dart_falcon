//! Portable CI reports with project-relative paths and shared baseline identities.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::Path;

use falcon_diagnostics::{Diagnostic, Severity, byte_to_line_col};
use serde_json::{Value, json};

use crate::analyze_pipeline::OutputFormat;

/// Render diagnostics without changing their order or the original diagnostics.
///
/// SARIF and GitLab share baseline fingerprints. Source files are read at most
/// once; unavailable sources use the baseline's empty-source fingerprint and
/// unresolved positions default to line/column one.
/// Repeated fingerprints receive occurrence suffixes (`:2`, `:3`, ...).
/// GitLab and Sonar paths are relative to the invocation directory; other report
/// paths and all baseline fingerprints use the supplied configuration root.
pub fn render(format: OutputFormat, diagnostics: &[Diagnostic], root: &Path) -> String {
    match format {
        OutputFormat::Text => return crate::output::format_text(diagnostics),
        OutputFormat::Json => return crate::output::format_json(diagnostics),
        _ => {}
    }
    let fingerprints = matches!(format, OutputFormat::Sarif | OutputFormat::Gitlab);
    let invocation_root = matches!(format, OutputFormat::Gitlab | OutputFormat::Sonar).then(|| {
        std::env::current_dir().expect("CI report paths require an accessible working directory")
    });
    let path_root = invocation_root.as_deref().unwrap_or(root);
    let mut sources = HashMap::with_capacity(diagnostics.len());
    let mut occurrences = HashMap::with_capacity(if fingerprints { diagnostics.len() } else { 0 });
    let entries: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            let source = if fingerprints || diagnostic.line == 0 || diagnostic.col == 0 {
                sources
                    .entry(diagnostic.file_path.as_str())
                    .or_insert_with(|| {
                        std::fs::read_to_string(&diagnostic.file_path).unwrap_or_default()
                    })
                    .as_str()
            } else {
                ""
            };
            let (line, col) = if diagnostic.line == 0 || diagnostic.col == 0 {
                byte_to_line_col(source, diagnostic.span.start)
            } else {
                (diagnostic.line, diagnostic.col)
            };
            Entry {
                diagnostic,
                path: crate::baseline::relative_path(&diagnostic.file_path, path_root),
                line,
                col,
                fingerprint: fingerprints.then(|| {
                    let base = crate::baseline::fingerprint(diagnostic, source, root);
                    let count = occurrences.entry(base.clone()).or_insert(0usize);
                    *count += 1;
                    if *count == 1 {
                        base
                    } else {
                        format!("{base}:{count}")
                    }
                }),
            }
        })
        .collect();
    match format {
        OutputFormat::Sarif => json_string(sarif(&entries, root)),
        OutputFormat::Gitlab => json_string(gitlab(&entries)),
        OutputFormat::Checkstyle => checkstyle(&entries),
        OutputFormat::Sonar => json_string(sonar(&entries)),
        OutputFormat::Html => html(&entries),
        OutputFormat::Text | OutputFormat::Json => unreachable!("handled before source loading"),
    }
}

struct Entry<'a> {
    diagnostic: &'a Diagnostic,
    path: String,
    line: u32,
    col: u32,
    fingerprint: Option<String>,
}

fn json_string(value: Value) -> String {
    serde_json::to_string_pretty(&value).expect("report values are JSON serializable")
}

fn sarif(entries: &[Entry<'_>], root: &Path) -> Value {
    let mut rules = BTreeMap::new();
    let results: Vec<_> = entries
        .iter()
        .map(|entry| {
            let diagnostic = entry.diagnostic;
            rules.entry(diagnostic.rule).or_insert_with(|| {
                let mut rule = json!({ "id": diagnostic.rule });
                if let Some(meta) = falcon_rules::meta::meta_for(diagnostic.rule) {
                    rule["properties"] = json!({
                        "tags": [meta.group.as_str()],
                        "recommended": meta.recommended,
                        "domains": meta.domain_names(),
                    });
                }
                rule
            });
            let level = match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            json!({
                "ruleId": diagnostic.rule,
                "level": level,
                "message": { "text": diagnostic.message },
                "partialFingerprints": { "falcon/v1": entry.fingerprint },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": uri_encode(&entry.path),
                            "uriBaseId": "%SRCROOT%",
                        },
                        "region": { "startLine": entry.line, "startColumn": entry.col },
                    },
                }],
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "falcon",
                "version": env!("CARGO_PKG_VERSION"),
                "rules": rules.into_values().collect::<Vec<_>>(),
            }},
            "columnKind": "utf16CodeUnits",
            "originalUriBaseIds": { "%SRCROOT%": { "uri": root_uri(root) } },
            "results": results,
        }],
    })
}

fn gitlab(entries: &[Entry<'_>]) -> Value {
    entries
        .iter()
        .map(|entry| {
            let severity = match entry.diagnostic.severity {
                Severity::Error => "critical",
                Severity::Warning => "major",
                Severity::Info => "info",
            };
            json!({
                "type": "issue",
                "check_name": entry.diagnostic.rule,
                "description": entry.diagnostic.message,
                "categories": ["Style"],
                "severity": severity,
                "fingerprint": entry.fingerprint,
                "location": { "path": entry.path, "lines": { "begin": entry.line } },
            })
        })
        .collect()
}

fn sonar(entries: &[Entry<'_>]) -> Value {
    let issues: Vec<_> = entries
        .iter()
        .map(|entry| {
            let severity = match entry.diagnostic.severity {
                Severity::Error => "CRITICAL",
                Severity::Warning => "MAJOR",
                Severity::Info => "INFO",
            };
            json!({
                "engineId": "falcon",
                "ruleId": entry.diagnostic.rule,
                "severity": severity,
                "type": "CODE_SMELL",
                "primaryLocation": {
                    "message": entry.diagnostic.message,
                    "filePath": entry.path,
                    "textRange": { "startLine": entry.line },
                },
            })
        })
        .collect();
    json!({ "issues": issues })
}

fn checkstyle(entries: &[Entry<'_>]) -> String {
    let mut output =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<checkstyle version=\"4.3\">\n");
    let mut files: BTreeMap<&str, Vec<&Entry<'_>>> = BTreeMap::new();
    for entry in entries {
        files.entry(&entry.path).or_default().push(entry);
    }
    for (path, entries) in files {
        writeln!(output, "  <file name=\"{}\">", escape_markup(path)).unwrap();
        for entry in entries {
            writeln!(
                output,
                "    <error line=\"{}\" column=\"{}\" severity=\"{}\" message=\"{}\" source=\"falcon.{}\"/>",
                entry.line,
                entry.col,
                entry.diagnostic.severity,
                escape_markup(&entry.diagnostic.message),
                escape_markup(entry.diagnostic.rule),
            )
            .unwrap();
        }
        output.push_str("  </file>\n");
    }
    output.push_str("</checkstyle>\n");
    output
}

fn html(entries: &[Entry<'_>]) -> String {
    let mut output = String::from(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>falcon report</title><style>\
         body{font-family:system-ui,sans-serif;margin:2rem;color:#222;background:#fff}\
         table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:.6rem;border:1px solid #ccc}\
         th{background:#eee}td{overflow-wrap:anywhere}\
         </style></head><body><h1>falcon report</h1>\
         <table><thead><tr><th scope=\"col\">File</th><th scope=\"col\">Line</th>\
         <th scope=\"col\">Column</th><th scope=\"col\">Severity</th>\
         <th scope=\"col\">Rule</th><th scope=\"col\">Message</th></tr></thead><tbody>",
    );
    for entry in entries {
        write!(
            output,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_markup(&entry.path),
            entry.line,
            entry.col,
            entry.diagnostic.severity,
            escape_markup(entry.diagnostic.rule),
            escape_markup(&entry.diagnostic.message),
        )
        .unwrap();
    }
    output.push_str("</tbody></table></body></html>\n");
    output
}

fn escape_markup(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' => escaped.push_str("&#9;"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            // XML 1.0 excludes these controls even as character references.
            '\0'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {
                escaped.push('\u{fffd}')
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn uri_encode(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "%{byte:02X}").unwrap();
        }
    }
    encoded
}

fn root_uri(root: &Path) -> String {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("report project root requires an accessible working directory")
            .join(root)
    };
    let path = absolute.to_string_lossy().replace('\\', "/");
    file_uri(&path)
}

fn file_uri(path: &str) -> String {
    let path = path.strip_prefix("//?/").unwrap_or(path);
    let encoded = uri_encode(path);
    let mut uri = if let Some(network_path) = path.strip_prefix("UNC/") {
        format!("file://{}", uri_encode(network_path))
    } else if path.starts_with("//") {
        format!("file:{encoded}")
    } else if path.starts_with('/') {
        format!("file://{encoded}")
    } else {
        // A Windows drive colon is a URI delimiter, unlike colons in filenames.
        format!("file:///{}", encoded.replacen("%3A", ":", 1))
    };
    if !uri.ends_with('/') {
        uri.push('/');
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze_pipeline::OutputFormat as ReportFormat;
    use falcon_diagnostics::Span;
    use serde_json::Value;

    fn diagnostic(root: &Path) -> Diagnostic {
        let mut diagnostic = Diagnostic::new(
            "avoid-print",
            Severity::Warning,
            "Avoid <&\"'print>.",
            root.join("lib/a &é.dart").to_string_lossy(),
            Span { start: 2, end: 7 },
        );
        diagnostic.line = 2;
        diagnostic.col = 3;
        diagnostic
    }

    #[test]
    fn sarif_has_base_uri_encoded_paths_rules_and_fingerprints() {
        let root = Path::new("/project with spaces");
        let diagnostic = diagnostic(root);
        let report: Value = serde_json::from_str(&render(
            ReportFormat::Sarif,
            std::slice::from_ref(&diagnostic),
            root,
        ))
        .unwrap();
        assert_eq!(report["version"], "2.1.0");
        let run = &report["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "falcon");
        assert_eq!(run["tool"]["driver"]["rules"][0]["id"], "avoid-print");
        assert!(run["tool"]["driver"]["rules"][0]["properties"]["tags"].is_array());
        assert_eq!(
            run["originalUriBaseIds"]["%SRCROOT%"]["uri"],
            "file:///project%20with%20spaces/"
        );
        let result = &run["results"][0];
        assert_eq!(result["level"], "warning");
        assert_eq!(result["message"]["text"], diagnostic.message);
        assert_eq!(
            result["partialFingerprints"]["falcon/v1"],
            crate::baseline::fingerprint(&diagnostic, "", root)
        );
        let location = &result["locations"][0]["physicalLocation"];
        assert_eq!(
            location["artifactLocation"]["uri"],
            "lib/a%20%26%C3%A9.dart"
        );
        assert_eq!(location["artifactLocation"]["uriBaseId"], "%SRCROOT%");
        assert_eq!(location["region"]["startLine"], 2);
        assert_eq!(location["region"]["startColumn"], 3);
    }

    #[test]
    fn gitlab_has_codeclimate_fields_and_shared_fingerprint() {
        let cwd = std::env::current_dir().unwrap();
        let root = cwd.as_path();
        let diagnostic = diagnostic(root);
        let report: Value = serde_json::from_str(&render(
            ReportFormat::Gitlab,
            std::slice::from_ref(&diagnostic),
            root,
        ))
        .unwrap();
        assert_eq!(report[0]["type"], "issue");
        assert_eq!(report[0]["check_name"], "avoid-print");
        assert_eq!(report[0]["description"], diagnostic.message);
        assert_eq!(report[0]["severity"], "major");
        assert_eq!(report[0]["location"]["path"], "lib/a &é.dart");
        assert_eq!(report[0]["location"]["lines"]["begin"], 2);
        assert_eq!(
            report[0]["fingerprint"],
            crate::baseline::fingerprint(&diagnostic, "", root)
        );
    }

    #[test]
    fn sonar_has_generic_issue_contract() {
        let cwd = std::env::current_dir().unwrap();
        let root = cwd.as_path();
        let report: Value =
            serde_json::from_str(&render(ReportFormat::Sonar, &[diagnostic(root)], root)).unwrap();
        let issue = &report["issues"][0];
        assert_eq!(issue["engineId"], "falcon");
        assert_eq!(issue["ruleId"], "avoid-print");
        assert_eq!(issue["severity"], "MAJOR");
        assert_eq!(issue["type"], "CODE_SMELL");
        assert_eq!(issue["primaryLocation"]["filePath"], "lib/a &é.dart");
        assert_eq!(issue["primaryLocation"]["textRange"]["startLine"], 2);
    }

    #[test]
    fn checkstyle_groups_files_and_escapes_all_attribute_delimiters() {
        let root = Path::new("/project");
        let diagnostic = diagnostic(root);
        let report = render(
            ReportFormat::Checkstyle,
            &[diagnostic.clone(), diagnostic],
            root,
        );
        assert!(report.starts_with(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<checkstyle version=\"4.3\">"
        ));
        assert_eq!(report.matches("<file ").count(), 1);
        assert_eq!(report.matches("<error ").count(), 2);
        assert!(report.contains("name=\"lib/a &amp;é.dart\""));
        assert!(report.contains("message=\"Avoid &lt;&amp;&quot;&apos;print&gt;.\""));
        assert!(report.contains("line=\"2\" column=\"3\" severity=\"warning\""));
        assert!(report.ends_with("</checkstyle>\n"));
    }

    #[test]
    fn html_is_self_contained_and_escapes_diagnostics() {
        let root = Path::new("/project");
        let report = render(ReportFormat::Html, &[diagnostic(root)], root);
        assert!(report.starts_with("<!doctype html>"));
        assert!(report.contains("<meta charset=\"utf-8\">"));
        assert!(report.contains("<style>"));
        assert!(report.contains("<tbody><tr>"));
        assert!(report.contains("Avoid &lt;&amp;&quot;&apos;print&gt;."));
        assert!(report.contains("lib/a &amp;é.dart"));
        assert!(!report.contains("<script"));
        assert!(!report.contains("href="));
        assert!(report.ends_with("</html>\n"));
    }

    #[test]
    fn empty_reports_are_valid_documents() {
        let root = Path::new("/project");
        let sarif: Value = serde_json::from_str(&render(ReportFormat::Sarif, &[], root)).unwrap();
        assert_eq!(sarif["runs"][0]["results"], serde_json::json!([]));
        assert_eq!(render(ReportFormat::Gitlab, &[], root), "[]");
        let sonar: Value = serde_json::from_str(&render(ReportFormat::Sonar, &[], root)).unwrap();
        assert_eq!(sonar["issues"], serde_json::json!([]));
        assert!(render(ReportFormat::Checkstyle, &[], root).contains("</checkstyle>"));
        assert!(render(ReportFormat::Html, &[], root).contains("<tbody></tbody>"));
    }

    #[test]
    fn report_reads_source_for_unresolved_positions_and_fingerprints() {
        let cwd = std::env::current_dir().unwrap();
        let root = tempfile::tempdir_in(&cwd).unwrap();
        let file = root.path().join("main.dart");
        std::fs::write(&file, "a\nprint('x');").unwrap();
        let diagnostic = Diagnostic::new(
            "avoid-print",
            Severity::Error,
            "Avoid print.",
            file.strip_prefix(&cwd).unwrap().to_string_lossy(),
            Span { start: 2, end: 7 },
        );
        let report: Value = serde_json::from_str(&render(
            ReportFormat::Sarif,
            std::slice::from_ref(&diagnostic),
            root.path(),
        ))
        .unwrap();
        let result = &report["runs"][0]["results"][0];
        assert_eq!(
            result["locations"][0]["physicalLocation"]["region"]["startLine"],
            2
        );
        assert_eq!(
            result["partialFingerprints"]["falcon/v1"],
            crate::baseline::fingerprint(&diagnostic, "a\nprint('x');", root.path())
        );
        assert_eq!(result["level"], "error");
    }

    #[test]
    fn severity_mappings_include_information_and_errors() {
        let root = Path::new("/project");
        let mut error = diagnostic(root);
        error.severity = Severity::Error;
        let mut info = diagnostic(root);
        info.severity = Severity::Info;
        let diagnostics = [error, info];
        let gitlab: Value =
            serde_json::from_str(&render(ReportFormat::Gitlab, &diagnostics, root)).unwrap();
        assert_eq!(gitlab[0]["severity"], "critical");
        assert_eq!(gitlab[1]["severity"], "info");
        let sonar: Value =
            serde_json::from_str(&render(ReportFormat::Sonar, &diagnostics, root)).unwrap();
        assert_eq!(sonar["issues"][0]["severity"], "CRITICAL");
        assert_eq!(sonar["issues"][1]["severity"], "INFO");
        let sarif: Value =
            serde_json::from_str(&render(ReportFormat::Sarif, &diagnostics, root)).unwrap();
        assert_eq!(sarif["runs"][0]["results"][1]["level"], "note");
    }

    #[test]
    fn markup_preserves_whitespace_and_replaces_invalid_xml_characters() {
        assert_eq!(
            escape_markup("<&\"'\t\n\r>\0\u{b}\u{fffe}"),
            "&lt;&amp;&quot;&apos;&#9;&#10;&#13;&gt;\u{fffd}\u{fffd}\u{fffd}",
        );
    }

    #[test]
    fn internal_diagnostics_do_not_require_rule_metadata_or_source_files() {
        let root = Path::new("/project");
        let diagnostic = Diagnostic::new(
            "config",
            Severity::Error,
            "Cannot load configuration.",
            "falcon.json",
            Span { start: 0, end: 0 },
        );
        let report: Value = serde_json::from_str(&render(
            ReportFormat::Sarif,
            std::slice::from_ref(&diagnostic),
            root,
        ))
        .unwrap();
        assert_eq!(
            report["runs"][0]["tool"]["driver"]["rules"][0],
            json!({"id": "config"})
        );
        assert_eq!(
            report["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]["startLine"],
            1
        );
        assert_eq!(
            render(OutputFormat::Text, std::slice::from_ref(&diagnostic), root),
            crate::output::format_text(std::slice::from_ref(&diagnostic)),
        );
        assert_eq!(
            render(OutputFormat::Json, std::slice::from_ref(&diagnostic), root),
            crate::output::format_json(std::slice::from_ref(&diagnostic)),
        );
    }

    #[test]
    fn repeated_fingerprints_remain_distinct_and_survive_line_shifts() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("main.dart");
        let source = "print\nprint\nprint";
        std::fs::write(&path, source).unwrap();
        let mut diagnostics: Vec<_> = [0, 6, 12]
            .into_iter()
            .map(|start| {
                Diagnostic::new(
                    "avoid-print",
                    Severity::Warning,
                    "Avoid print.",
                    path.to_string_lossy(),
                    Span {
                        start,
                        end: start + 5,
                    },
                )
            })
            .collect();
        let base = crate::baseline::fingerprint(&diagnostics[0], source, root.path());
        let expected = vec![base.clone(), format!("{base}:2"), format!("{base}:3")];
        let fingerprints = |format, diagnostics: &[Diagnostic]| {
            let report: Value =
                serde_json::from_str(&render(format, diagnostics, root.path())).unwrap();
            match format {
                OutputFormat::Gitlab => report
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|issue| issue["fingerprint"].as_str().unwrap().to_owned())
                    .collect::<Vec<_>>(),
                OutputFormat::Sarif => report["runs"][0]["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|issue| {
                        issue["partialFingerprints"]["falcon/v1"]
                            .as_str()
                            .unwrap()
                            .to_owned()
                    })
                    .collect(),
                _ => unreachable!(),
            }
        };
        assert_eq!(fingerprints(OutputFormat::Gitlab, &diagnostics), expected);
        assert_eq!(fingerprints(OutputFormat::Sarif, &diagnostics), expected);
        std::fs::write(&path, format!("\n{source}")).unwrap();
        for diagnostic in &mut diagnostics {
            diagnostic.span.start += 1;
            diagnostic.span.end += 1;
        }
        assert_eq!(fingerprints(OutputFormat::Gitlab, &diagnostics), expected);
        assert_eq!(fingerprints(OutputFormat::Sarif, &diagnostics), expected);
    }

    #[test]
    fn file_uris_support_windows_verbatim_drives_and_network_roots() {
        assert_eq!(
            file_uri("C:/project with spaces"),
            "file:///C:/project%20with%20spaces/"
        );
        assert_eq!(file_uri("//?/C:/project"), "file:///C:/project/");
        assert_eq!(
            file_uri("//server/share/project"),
            "file://server/share/project/"
        );
        assert_eq!(
            file_uri("//?/UNC/server/share/project"),
            "file://server/share/project/"
        );
    }

    #[test]
    fn gitlab_and_sonar_paths_use_invocation_root_not_nested_configuration_root() {
        let cwd = std::env::current_dir().unwrap();
        let project = tempfile::tempdir_in(&cwd).unwrap();
        let root = project.path().join("nested");
        std::fs::create_dir(&root).unwrap();
        let diagnostic = diagnostic(&root);
        let expected = Path::new(&diagnostic.file_path)
            .strip_prefix(&cwd)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let gitlab: Value = serde_json::from_str(&render(
            OutputFormat::Gitlab,
            std::slice::from_ref(&diagnostic),
            &root,
        ))
        .unwrap();
        assert_eq!(gitlab[0]["location"]["path"], expected);
        assert_eq!(
            gitlab[0]["fingerprint"],
            crate::baseline::fingerprint(&diagnostic, "", &root),
        );
        let sonar: Value = serde_json::from_str(&render(
            OutputFormat::Sonar,
            std::slice::from_ref(&diagnostic),
            &root,
        ))
        .unwrap();
        assert_eq!(sonar["issues"][0]["primaryLocation"]["filePath"], expected);
    }
}
