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
    /// 1-based line of `span.start`, resolved from source via
    /// [`Diagnostic::resolve_position`]. 0 means unresolved.
    pub line: u32,
    /// 1-based column of `span.start`. 0 means unresolved.
    pub col: u32,
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

/// Convert an LSP `Position` (0-based line + UTF-16 `character`) back to a byte
/// offset in `source` — the inverse of [`byte_to_lsp_position`].
///
/// Positions past the end of a line clamp to that line's newline; positions
/// past the last line clamp to `source.len()`. `character` counts UTF-16 code
/// units, per the LSP default encoding (astral chars = 2 units), so a position
/// landing inside a surrogate pair resolves to the character's start.
pub fn lsp_position_to_byte(source: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut character = 0u32;
    for (i, c) in source.char_indices() {
        if line == position.line && character >= position.character {
            return i;
        }
        if line > position.line {
            // Requested character was past the end of its line; clamp there.
            return i.saturating_sub(1);
        }
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            let next = character + c.len_utf16() as u32;
            // Target lands inside this char (e.g. mid surrogate pair): clamp to
            // its start rather than splitting the code point.
            if line == position.line && next > position.character {
                return i;
            }
            character = next;
        }
    }
    source.len()
}

/// Convert a byte offset in `source` to an LSP `Position` (0-based line +
/// UTF-16 `character`, the LSP default encoding — astral chars count as 2).
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
            character += c.len_utf16() as u32;
        }
    }
    Position { line, character }
}

/// 1-based `(line, column)` of `offset` within `source` — the convention
/// `dart analyze` and editor jump-to-error use. Columns count Unicode scalar
/// values (consistent with [`byte_to_lsp_position`]).
pub fn byte_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let pos = byte_to_lsp_position(source, offset);
    (pos.line + 1, pos.character + 1)
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
            line: 0,
            col: 0,
            suggestions: Vec::new(),
            context_lines: Vec::new(),
        }
    }

    /// Fill `line`/`col` (1-based) from `source` for this diagnostic's span start.
    pub fn resolve_position(&mut self, source: &str) {
        let (line, col) = byte_to_line_col(source, self.span.start);
        self.line = line;
        self.col = col;
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
            "{}:{}:{}: [{}] ({}) {}",
            self.file_path, self.line, self.col, self.severity, self.rule, self.message
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
            source: Some("falcon".to_string()),
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

impl std::str::FromStr for Severity {
    type Err = String;

    /// Parse a severity name as written in `falcon.json` `severity_override`
    /// entries. Accepts the same names `Display` produces, plus "warn".
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" | "warn" => Ok(Severity::Warning),
            "info" => Ok(Severity::Info),
            "note" => Ok(Severity::Note),
            other => Err(format!("unknown severity: {other}")),
        }
    }
}

#[cfg(test)]
mod position_tests {
    use super::{byte_to_lsp_position, lsp_position_to_byte};
    use lsp_types::Position;

    // `😀` is U+1F600: 4 UTF-8 bytes, 1 scalar, 2 UTF-16 code units.
    const EMOJI: &str = "😀";

    #[test]
    fn character_counts_utf16_units_past_astral_char() {
        let src = "a😀b";
        // byte offsets: 'a'=0, emoji=1..5, 'b'=5
        assert_eq!(
            byte_to_lsp_position(src, 5),
            Position {
                line: 0,
                character: 3
            },
            "'b' sits at UTF-16 column 3 (1 for 'a' + 2 for the emoji)"
        );
    }

    #[test]
    fn character_on_bmp_text_is_unchanged() {
        let src = "abc\ndef";
        assert_eq!(
            byte_to_lsp_position(src, 5),
            Position {
                line: 1,
                character: 1
            }
        );
    }

    #[test]
    fn round_trips_through_astral_chars() {
        let src = "  var s = \"😀😀\"; var x = 1;";
        let x_byte = src.find("var x").unwrap();
        let pos = byte_to_lsp_position(src, x_byte);
        // Two emoji before `var x`: each is 2 UTF-16 units, not 1 scalar.
        assert_eq!(pos.character, 18);
        assert_eq!(lsp_position_to_byte(src, pos), x_byte);
    }

    #[test]
    fn position_inside_surrogate_pair_clamps_to_char_start() {
        let src = "a😀b";
        // Character 2 lands between the emoji's two UTF-16 units; resolve to the
        // emoji's byte start rather than splitting it.
        let byte = lsp_position_to_byte(
            src,
            Position {
                line: 0,
                character: 2,
            },
        );
        assert_eq!(byte, 1, "emoji starts at byte 1");
    }

    #[test]
    fn lsp_position_to_byte_lands_after_emoji() {
        let src = format!("x{EMOJI}y");
        // 'y' is UTF-16 character 3 (1 + 2).
        let byte = lsp_position_to_byte(
            &src,
            Position {
                line: 0,
                character: 3,
            },
        );
        assert_eq!(&src[byte..], "y");
    }
}
