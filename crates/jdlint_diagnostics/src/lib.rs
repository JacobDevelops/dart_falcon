//! Diagnostic types, severity levels, and reporting.
//!
//! The `Diagnostic` type is the canonical output of every lint rule.

use lsp_types::{Diagnostic as LspDiagnostic, DiagnosticSeverity, Position, Range};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
}

/// A single source line displayed alongside a diagnostic for context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLine {
    pub line_number: u32,
    pub content: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub file_path: String,
    pub span: Span,
    pub suggestions: Vec<Suggestion>,
    pub context_lines: Vec<ContextLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Note,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Convert a byte offset in `source` to an LSP `Position` (0-based line + character).
pub fn byte_to_lsp_position(source: &str, offset: usize) -> Position {
    let clamped = offset.min(source.len());
    let mut line = 0u32;
    let mut character = 0u32;
    for (i, c) in source.char_indices() {
        if i >= clamped {
            break;
        }
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Position { line, character }
}

impl Diagnostic {
    pub fn new(
        rule: &'static str,
        severity: Severity,
        message: impl Into<String>,
        file_path: impl Into<String>,
        span: Span,
    ) -> Self {
        let msg = message.into();
        Self {
            rule,
            code: rule.to_string(),
            severity,
            message: msg,
            file_path: file_path.into(),
            span,
            suggestions: Vec::new(),
            context_lines: Vec::new(),
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn with_suggestion(mut self, msg: impl Into<String>, replacement: Option<String>) -> Self {
        self.suggestions.push(Suggestion {
            message: msg.into(),
            replacement,
        });
        self
    }

    pub fn with_context_lines(mut self, lines: Vec<ContextLine>) -> Self {
        self.context_lines = lines;
        self
    }

    pub fn format_text(&self) -> String {
        format!(
            "{}:+{}:{}: [{}] ({}) {}",
            self.file_path, self.span.start, self.span.end, self.severity, self.rule, self.message
        )
    }

    pub fn format_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }

    /// Serialize to an LSP `Diagnostic`, computing line/column from `source`.
    ///
    /// `source` is the full text of the file — required to convert the byte-offset
    /// `Span` into the 0-based line+character positions that LSP expects.
    pub fn format_lsp(&self, source: &str) -> LspDiagnostic {
        let start_pos = byte_to_lsp_position(source, self.span.start);
        let end_pos = byte_to_lsp_position(source, self.span.end);
        LspDiagnostic {
            range: Range {
                start: start_pos,
                end: end_pos,
            },
            severity: Some(match self.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Info | Severity::Note => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(lsp_types::NumberOrString::String(self.code.clone())),
            source: Some("jdlint".to_string()),
            message: self.message.clone(),
            related_information: None,
            tags: None,
            code_description: None,
            data: None,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Note => write!(f, "note"),
        }
    }
}
