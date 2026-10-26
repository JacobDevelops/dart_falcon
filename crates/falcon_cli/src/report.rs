//! Portable CI reports with project-relative paths and shared baseline identities.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use falcon_diagnostics::{Diagnostic, Severity, byte_to_line_col};
use serde_json::{Value, json};

use crate::analyze_pipeline::OutputFormat;

/// Counts for a workspace member, including members with no findings.
#[derive(Debug)]
pub struct PackageSummary {
    pub name: String,
    pub root: PathBuf,
    pub total_files: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

struct PackageAttribution<'a> {
    packages: &'a [PackageSummary],
    cwd: PathBuf,
    paths: HashMap<PathBuf, Option<usize>>,
    directories: HashMap<PathBuf, Option<usize>>,
}

impl<'a> PackageAttribution<'a> {
    fn new(packages: &'a [PackageSummary], capacity: usize) -> Self {
        Self {
            packages,
            cwd: std::env::current_dir().expect("report paths require a working directory"),
            paths: HashMap::with_capacity(capacity),
            directories: HashMap::with_capacity(capacity),
        }
    }

    fn index(&mut self, path: &Path) -> Option<usize> {
        if let Some(index) = self.paths.get(path) {
            return *index;
        }
        let absolute = if path.is_absolute() {
            falcon_analyze::canonical_or_lexical(path)
        } else {
            falcon_analyze::canonical_or_lexical(&self.cwd.join(path))
        };
        let directory = if absolute.is_dir() {
            absolute.as_path()
        } else {
            absolute.parent()?
        };
        let index = *self
            .directories
            .entry(directory.to_path_buf())
            .or_insert_with(|| {
                // A nested, non-member pubspec is a package boundary too.
                if let Some(package_root) = directory
                    .ancestors()
                    .find(|ancestor| ancestor.join("pubspec.yaml").is_file())
                {
                    self.packages
                        .iter()
                        .position(|package| package.root == package_root)
                } else {
                    self.packages
                        .iter()
                        .enumerate()
                        .filter(|(_, package)| absolute.starts_with(&package.root))
                        .max_by_key(|(_, package)| package.root.components().count())
                        .map(|(index, _)| index)
                }
            });
        self.paths.insert(path.to_path_buf(), index);
        index
    }
}

/// Initialize selected member file counts and optional unbaselined findings.
pub fn summarize_packages<'a>(
    packages: &[falcon_project::WorkspacePackage],
    files: impl IntoIterator<Item = &'a Path>,
    diagnostics: &[Diagnostic],
) -> Vec<PackageSummary> {
    let mut summaries: Vec<_> = packages
        .iter()
        .map(|package| PackageSummary {
            name: package.name.clone(),
            root: falcon_analyze::canonical_or_lexical(&package.root),
            total_files: 0,
            errors: 0,
            warnings: 0,
            infos: 0,
        })
        .collect();
    let mut attribution = PackageAttribution::new(&summaries, summaries.len());
    let mut file_counts = vec![0; summaries.len()];
    for path in files {
        if let Some(index) = attribution.index(path) {
            // Counts are applied after releasing the attribution's shared borrow.
            file_counts[index] += 1;
        }
    }
    drop(attribution);
    for (summary, count) in summaries.iter_mut().zip(file_counts) {
        summary.total_files = count;
    }
    count_package_diagnostics(&mut summaries, diagnostics);
    summaries
}

pub(crate) fn count_package_diagnostics(
    summaries: &mut [PackageSummary],
    diagnostics: &[Diagnostic],
) {
    if summaries.is_empty() || diagnostics.is_empty() {
        return;
    }
    let mut attribution = PackageAttribution::new(summaries, diagnostics.len());
    let indices: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| attribution.index(Path::new(&diagnostic.file_path)))
        .collect();
    for (diagnostic, index) in diagnostics.iter().zip(indices) {
        if let Some(index) = index {
            let summary = &mut summaries[index];
            match diagnostic.severity {
                Severity::Error => summary.errors += 1,
                Severity::Warning => summary.warnings += 1,
                Severity::Info => summary.infos += 1,
            }
        }
    }
}

/// Render workspace metadata without changing legacy single-project reports.
pub fn render_with_packages(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    root: &Path,
    packages: &[PackageSummary],
) -> String {
    if packages.is_empty() {
        return render(format, diagnostics, root);
    }
    if !matches!(format, OutputFormat::Text | OutputFormat::Json) {
        return render_impl(format, diagnostics, root, packages);
    }
    let mut attribution = PackageAttribution::new(packages, diagnostics.len());
    let indices: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| attribution.index(Path::new(&diagnostic.file_path)))
        .collect();
    if format == OutputFormat::Text {
        let mut counts = vec![0; packages.len() + 1];
        for index in &indices {
            counts[index.unwrap_or(packages.len())] += 1;
        }
        let mut groups: Vec<Vec<&Diagnostic>> =
            counts.into_iter().map(Vec::with_capacity).collect();
        for (diagnostic, index) in diagnostics.iter().zip(indices) {
            groups[index.unwrap_or(packages.len())].push(diagnostic);
        }
        let mut output = String::new();
        for (index, package) in packages.iter().enumerate() {
            writeln!(output, "Package {}:", package.name).unwrap();
            for diagnostic in &groups[index] {
                writeln!(output, "{}", diagnostic.format_text()).unwrap();
            }
        }
        let outside = &groups[packages.len()];
        if !outside.is_empty() {
            output.push_str("Outside workspace packages:\n");
            for diagnostic in outside {
                writeln!(output, "{}", diagnostic.format_text()).unwrap();
            }
        }
        output.push_str("Package summary:\n");
        for package in packages {
            writeln!(
                output,
                "  {}: {} files, {} errors, {} warnings, {} infos",
                package.name, package.total_files, package.errors, package.warnings, package.infos,
            )
            .unwrap();
        }
        output.pop();
        return output;
    }
    if format == OutputFormat::Json {
        let entries: Vec<_> = diagnostics
            .iter()
            .zip(&indices)
            .map(|(diagnostic, index)| {
                let mut entry =
                    serde_json::to_value(diagnostic).expect("diagnostics are JSON serializable");
                entry["package"] = json!(index.map(|index| packages[index].name.as_str()));
                entry
            })
            .collect();
        // Keep diagnostic fields unchanged; the summary follows the findings.
        let mut report = serde_json::Map::new();
        report.insert("diagnostics".into(), entries.into());
        report.insert("packages".into(), package_values(packages, root).into());
        return json_string(report.into());
    }
    render_impl(format, diagnostics, root, packages)
}

fn package_values(packages: &[PackageSummary], root: &Path) -> Vec<Value> {
    packages
        .iter()
        .map(|package| {
            json!({
                "name": package.name,
                "root": crate::baseline::relative_path(&package.root.to_string_lossy(), root),
                "total_files": package.total_files,
                "errors": package.errors,
                "warnings": package.warnings,
                "infos": package.infos,
            })
        })
        .collect()
}

/// Render diagnostics without changing their order or the original diagnostics.
///
/// SARIF and GitLab share baseline fingerprints. Source files are read at most
/// once; unavailable sources use the baseline's empty-source fingerprint and
/// unresolved positions default to line/column one.
/// Repeated fingerprints receive occurrence suffixes (`:2`, `:3`, ...).
/// GitLab and Sonar paths are relative to the invocation directory; other report
/// paths and all baseline fingerprints use the supplied configuration root.
pub fn render(format: OutputFormat, diagnostics: &[Diagnostic], root: &Path) -> String {
    render_impl(format, diagnostics, root, &[])
}

fn render_impl(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    root: &Path,
    packages: &[PackageSummary],
) -> String {
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
        OutputFormat::Sarif => json_string(sarif(&entries, root, packages)),
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

fn sarif(entries: &[Entry<'_>], root: &Path, packages: &[PackageSummary]) -> Value {
    let mut rules = BTreeMap::new();
    let capacity = if packages.is_empty() {
        0
    } else {
        entries.len()
    };
    let mut attribution = PackageAttribution::new(packages, capacity);
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
            let mut result = json!({
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
            });
            if !packages.is_empty() {
                let package = attribution
                    .index(Path::new(&diagnostic.file_path))
                    .map(|index| packages[index].name.as_str());
                result["properties"] = json!({ "package": package });
            }
            result
        })
        .collect();
    let mut report = json!({
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
    });
    if !packages.is_empty() {
        report["runs"][0]["properties"] = json!({ "packages": package_values(packages, root) });
    }
    report
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

    #[test]
    fn package_counts_use_deepest_member_and_retain_empty_members() {
        let root = Path::new("/workspace");
        let packages = [
            falcon_project::WorkspacePackage {
                name: "root".into(),
                root: root.to_path_buf(),
            },
            falcon_project::WorkspacePackage {
                name: "nested".into(),
                root: root.join("packages/nested"),
            },
            falcon_project::WorkspacePackage {
                name: "empty".into(),
                root: root.join("packages/empty"),
            },
        ];
        let diagnostics = [diagnostic(&packages[1].root)];
        let files = [
            root.join("lib/root.dart"),
            packages[1].root.join("lib/main.dart"),
        ];
        let summaries =
            summarize_packages(&packages, files.iter().map(PathBuf::as_path), &diagnostics);
        assert_eq!(summaries[0].total_files, 1);
        assert_eq!(summaries[0].warnings, 0);
        assert_eq!(summaries[1].total_files, 1);
        assert_eq!(summaries[1].warnings, 1);
        assert_eq!(summaries[2].total_files, 0);
        assert_eq!(summaries[2].warnings, 0);
        let report: Value = serde_json::from_str(&render_with_packages(
            ReportFormat::Json,
            &diagnostics,
            root,
            &summaries,
        ))
        .unwrap();
        assert_eq!(report["diagnostics"][0]["package"], "nested");
        assert_eq!(report["packages"][1]["root"], "packages/nested");
        assert_eq!(report["packages"][2]["total_files"], 0);
    }

    #[test]
    fn reports_without_workspace_keep_legacy_json_contract() {
        let diagnostics = [diagnostic(Path::new("/project"))];
        assert_eq!(
            render_with_packages(ReportFormat::Json, &diagnostics, Path::new("/project"), &[]),
            crate::output::format_json(&diagnostics),
        );
    }

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
