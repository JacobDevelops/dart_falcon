//! Diagnostic output formatting: text and JSON.

use falcon_diagnostics::Diagnostic;

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
    serde_json::to_string_pretty(diagnostics).expect("diagnostics are JSON serializable")
}
