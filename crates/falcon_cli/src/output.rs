//! Diagnostic output formatting: text and JSON.

use falcon_diagnostics::Diagnostic;

/// Format diagnostics as human-readable text lines, joined by newlines.
#[must_use]
pub fn format_text(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(Diagnostic::format_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format diagnostics as a JSON array string.
///
/// # Panics
/// Panics if diagnostics contain a value that cannot be serialized.
#[must_use]
pub fn format_json(diagnostics: &[Diagnostic]) -> String {
    serde_json::to_string_pretty(diagnostics).expect("diagnostics are JSON serializable")
}

#[cfg(test)]
mod tests {
    use super::format_json;
    use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic, Severity, Span};

    #[test]
    fn nonempty_json_output_contract() {
        let mut resolved = Diagnostic::new(
            "avoid-print",
            Severity::Warning,
            "Avoid `print`.",
            "lib/main.dart",
            Span { start: 2, end: 7 },
        );
        resolved.resolve_position("a\nprint('hi');");
        let unresolved = Diagnostic::new(
            CONFIG_DIAGNOSTIC_ID,
            Severity::Error,
            "Cannot load configuration.",
            "falcon.json",
            Span { start: 0, end: 0 },
        );
        assert_eq!(
            format_json(&[resolved, unresolved]),
            r#"[
  {
    "rule": "avoid-print",
    "severity": "warning",
    "message": "Avoid `print`.",
    "file_path": "lib/main.dart",
    "span": {
      "start": 2,
      "end": 7
    },
    "line": 2,
    "col": 1
  },
  {
    "rule": "config",
    "severity": "error",
    "message": "Cannot load configuration.",
    "file_path": "falcon.json",
    "span": {
      "start": 0,
      "end": 0
    }
  }
]"#
        );
    }
}
