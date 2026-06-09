//! Diagnostic output formatting: text, JSON, quiet.

use jdlint_diagnostics::Diagnostic;

/// Format diagnostics as human-readable text lines, joined by newlines.
pub fn format_text(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.format_text())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format diagnostics as a JSON array string.
pub fn format_json(diagnostics: &[Diagnostic]) -> String {
    let values: Vec<_> = diagnostics.iter().map(|d| d.format_json()).collect();
    serde_json::to_string_pretty(&values).unwrap_or_else(|_| "[]".to_string())
}

/// Format diagnostics in quiet mode: no output (exit code communicates result).
pub fn format_quiet(_diagnostics: &[Diagnostic]) -> String {
    String::new()
}
