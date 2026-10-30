//! Shared fixture discovery and same-line expectation syntax for both corpus runners.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Expectation {
    pub rule: String,
    pub line: usize,
    pub expected_msg: Option<String>,
}

/// Ordinary fixtures are recursive, but project fixtures must run together.
pub fn ordinary_fixture_files(rule_dir: &Path, resolver_dependent: bool) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(rule_dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() != 1
                || !entry.file_type().is_dir()
                || (entry.file_name() != "cross-file"
                    && (!resolver_dependent || entry.file_name() != "project"))
        })
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("cannot walk {}: {error}", rule_dir.display()))
        })
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dart")
        })
        .filter(|path| {
            if path.parent() == Some(rule_dir) {
                return true;
            }
            if path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| {
                    stem.starts_with("bad") || stem.starts_with("good") || stem.starts_with("clean")
                })
            {
                return true;
            }
            let rule = rule_dir.file_name().and_then(|name| name.to_str());
            let source = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            parse_expectations(&source)
                .iter()
                .any(|expectation| Some(expectation.rule.as_str()) == rule)
        })
        .collect();
    files.sort();
    files
}

pub fn parse_expectations(source: &str) -> Vec<Expectation> {
    let mut expectations = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let mut search = line;
        while let Some(start) = search.find("/* expect:") {
            let after = &search[start + "/* expect:".len()..];
            let Some(end) = after.find("*/") else { break };
            let mut parts = after[..end].splitn(2, ',');
            let rule = parts.next().unwrap_or("").trim();
            let expected_msg = parts
                .next()
                .and_then(|part| part.trim().strip_prefix("msg:"))
                .map(|text| {
                    let text = text.trim();
                    text.strip_prefix('"')
                        .and_then(|text| text.strip_suffix('"'))
                        .unwrap_or(text)
                        .to_owned()
                });
            if !rule.is_empty() {
                expectations.push(Expectation {
                    rule: rule.to_owned(),
                    line: line_idx + 1,
                    expected_msg,
                });
            }
            search = &after[end + 2..];
        }
    }
    expectations
}

#[cfg(test)]
pub fn contains_placeholder_marker(source: &str) -> bool {
    source.lines().any(|line| {
        matches!(
            line.trim(),
            "// Placeholder fixture: filled in when the rule is implemented."
                | "// Placeholder: compliant code, no annotations."
                | "// Placeholder: violation examples with expect annotations will be added"
        )
    })
}

pub fn cross_file_project_roots(dir: &Path) -> Vec<PathBuf> {
    let mut projects: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("bad") || name.starts_with("good"))
        })
        .collect();
    projects.sort();
    if projects.is_empty() {
        projects.push(dir.to_path_buf());
    }
    projects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_fixtures_recurse_without_isolating_project_inputs() {
        let directory = tempfile::tempdir().unwrap();
        for relative in [
            "good.dart",
            "nested/deeper/bad.dart",
            "cross-file/bad/main.dart",
            "project/lib/good.dart",
            "packages/flutter/lib/widgets.dart",
            "flutter/lib/material.dart",
            "riverpod/lib/riverpod.dart",
            "dependency/lib/types.dart",
            "support/helper.dart",
            "targets/library.dart",
        ] {
            let path = directory.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "void main() {}").unwrap();
        }
        let relative_files = |resolved| {
            ordinary_fixture_files(directory.path(), resolved)
                .into_iter()
                .map(|path| path.strip_prefix(directory.path()).unwrap().to_path_buf())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            relative_files(true),
            vec![
                PathBuf::from("good.dart"),
                PathBuf::from("nested/deeper/bad.dart")
            ]
        );
        assert_eq!(
            relative_files(false),
            vec![
                PathBuf::from("good.dart"),
                PathBuf::from("nested/deeper/bad.dart"),
                PathBuf::from("project/lib/good.dart"),
            ]
        );
    }

    #[test]
    fn nested_annotations_opt_in_only_for_the_owning_rule() {
        let directory = tempfile::tempdir().unwrap();
        let rule_dir = directory.path().join("sample");
        for (relative, source) in [
            (
                "nested/example.dart",
                "x /* expect: sample, msg: \"Use `x`.\" */",
            ),
            ("support/helper.dart", "x /* expect: unrelated */"),
            ("nested/plain.dart", "void main() {}"),
            ("nested/clean_example.dart", "void main() {}"),
        ] {
            let path = rule_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        let files: Vec<_> = ordinary_fixture_files(&rule_dir, false)
            .into_iter()
            .map(|path| path.strip_prefix(&rule_dir).unwrap().to_path_buf())
            .collect();
        assert_eq!(
            files,
            vec![
                PathBuf::from("nested/clean_example.dart"),
                PathBuf::from("nested/example.dart"),
            ]
        );
    }

    #[test]
    fn parses_multiple_markers_and_messages_without_inventing_columns() {
        let expectations = parse_expectations(
            "x /* expect: first, msg: \"text, with comma\" */ /* expect: second */\n/* expect: */\n/* expect: unfinished",
        );
        assert_eq!(expectations.len(), 2);
        assert_eq!(expectations[0].rule, "first");
        assert_eq!(expectations[0].line, 1);
        assert_eq!(
            expectations[0].expected_msg.as_deref(),
            Some("text, with comma")
        );
        assert_eq!(expectations[1].rule, "second");
        assert_eq!(expectations[1].line, 1);
        assert_eq!(expectations[1].expected_msg, None);
    }

    #[test]
    fn message_parser_preserves_quotes_inside_outer_delimiters() {
        let expectations = parse_expectations(
            r#"x /* expect: sample, msg: ""quoted"" */ /* expect: sample, msg: "Use `"value"`." */"#,
        );
        assert_eq!(expectations[0].expected_msg.as_deref(), Some("\"quoted\""));
        assert_eq!(
            expectations[1].expected_msg.as_deref(),
            Some("Use `\"value\"`.")
        );
    }
}
