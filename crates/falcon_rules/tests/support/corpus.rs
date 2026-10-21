//! Shared fixture discovery and same-line expectation syntax for both corpus runners.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Expectation {
    pub rule: String,
    pub line: usize,
    pub expected_msg: Option<String>,
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
                .map(|text| text.trim().trim_matches('"').to_owned());
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
        .filter_map(|entry| entry.ok())
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
}
