//! Diagnostic types, severity levels, and reporting.
//!
//! The `Diagnostic` type is the canonical output of every lint rule.
//! JSON exposes rule, severity, message, file_path, span, resolved positions, and an optional fix.
//! LSP diagnostics use the rule name as their code.

use lsp_types::{Diagnostic as LspDiagnostic, DiagnosticSeverity, Position, Range};
use serde::Serialize;
use std::fmt;

mod fix;
pub use fix::{ApplyFixError, Fix, FixKind, TextEdit};

pub const CONFIG_DIAGNOSTIC_ID: &str = "config";
pub const SYNTAX_ERROR_DIAGNOSTIC_ID: &str = "syntax-error";
pub const UNMATCHED_PATH_DIAGNOSTIC_ID: &str = "unmatched-path";
pub const RULES_THREAD_ERROR_DIAGNOSTIC_ID: &str = "rules-thread-error";
pub const MALFORMED_SUPPRESSION_DIAGNOSTIC_ID: &str = "malformed-suppression";
pub const AUTOFIX_DIAGNOSTIC_ID: &str = "autofix";
pub const RESERVED_DIAGNOSTIC_IDS: &[&str] = &[
    CONFIG_DIAGNOSTIC_ID,
    SYNTAX_ERROR_DIAGNOSTIC_ID,
    UNMATCHED_PATH_DIAGNOSTIC_ID,
    RULES_THREAD_ERROR_DIAGNOSTIC_ID,
    MALFORMED_SUPPRESSION_DIAGNOSTIC_ID,
    AUTOFIX_DIAGNOSTIC_ID,
];

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    pub file_path: String,
    pub span: Span,
    /// 1-based line of `span.start`, resolved from source via
    /// [`Diagnostic::resolve_position`]. 0 means unresolved.
    #[serde(skip_serializing_if = "is_zero")]
    pub line: u32,
    /// 1-based column of `span.start`. 0 means unresolved.
    #[serde(skip_serializing_if = "is_zero")]
    pub col: u32,
    /// An optional rule-declared fix over the original source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn contains(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub fn contains_offset(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }
}

impl From<falcon_syntax::cst::Span> for Span {
    fn from(span: falcon_syntax::cst::Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
        }
    }
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

/// 1-based `(line, column)` of `offset` within `source`.
/// Columns count UTF-16 code units, consistent with [`byte_to_lsp_position`].
pub fn byte_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let pos = byte_to_lsp_position(source, offset);
    (pos.line + 1, pos.character + 1)
}

/// Line starts of one source, so resolving many offsets in it does not
/// rescan the source from the beginning for each.
pub struct LineIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(source: &'a str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(newline, _)| newline + 1))
            .collect();
        Self {
            source,
            line_starts,
        }
    }

    /// The 0-based line containing a byte offset, clamped to the source length.
    pub fn line_for_offset(&self, offset: usize) -> u32 {
        let clamped = offset.min(self.source.len());
        // Line 0 starts at 0 <= clamped, so the partition point is at least 1.
        (self.line_starts.partition_point(|&start| start <= clamped) - 1) as u32
    }

    /// The same position as [`byte_to_lsp_position`] for this source.
    pub fn lsp_position(&self, offset: usize) -> Position {
        let clamped = offset.min(self.source.len());
        let line = self.line_for_offset(clamped) as usize;
        let start = self.line_starts[line];
        let character = self.source[start..]
            .char_indices()
            .take_while(|(index, _)| start + index < clamped)
            .map(|(_, c)| c.len_utf16() as u32)
            .sum();
        Position {
            line: line as u32,
            character,
        }
    }
}

impl Diagnostic {
    pub fn new(
        rule: &'static str,
        severity: Severity,
        message: impl Into<String>,
        file_path: impl Into<String>,
        span: impl Into<Span>,
    ) -> Self {
        Self {
            rule,
            severity,
            message: message.into(),
            file_path: file_path.into(),
            span: span.into(),
            line: 0,
            col: 0,
            fix: None,
        }
    }

    /// Attach a fix whose safety is explicitly declared by the rule.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Fill `line`/`col` (1-based) from `source` for this diagnostic's span start.
    pub fn resolve_position(&mut self, source: &str) {
        let (line, col) = byte_to_line_col(source, self.span.start);
        self.line = line;
        self.col = col;
    }

    /// [`Self::resolve_position`] against a prebuilt index of the source.
    pub fn resolve_position_in(&mut self, lines: &LineIndex<'_>) {
        let position = lines.lsp_position(self.span.start);
        self.line = position.line + 1;
        self.col = position.character + 1;
    }

    pub fn format_text(&self) -> String {
        if self.line == 0 || self.col == 0 {
            format!(
                "{}: [{}] ({}) {}",
                self.file_path, self.severity, self.rule, self.message
            )
        } else {
            format!(
                "{}:{}:{}: [{}] ({}) {}",
                self.file_path, self.line, self.col, self.severity, self.rule, self.message
            )
        }
    }

    /// Serialize to an LSP `Diagnostic`, computing line/column from `source`.
    ///
    /// `source` is the full text of the file — required to convert the byte-offset
    /// `Span` into the 0-based line+character positions that LSP expects.
    pub fn format_lsp(&self, source: &str) -> LspDiagnostic {
        self.format_lsp_in(&LineIndex::new(source))
    }

    /// Serialize with a source index shared by every diagnostic in one LSP publish.
    ///
    /// Positions use UTF-16 code units, including two units per astral character.
    pub fn format_lsp_in(&self, lines: &LineIndex<'_>) -> LspDiagnostic {
        let start_pos = lines.lsp_position(self.span.start);
        let end_pos = lines.lsp_position(self.span.end);
        LspDiagnostic {
            range: Range {
                start: start_pos,
                end: end_pos,
            },
            severity: Some(match self.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Info => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(lsp_types::NumberOrString::String(self.rule.to_owned())),
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
        }
    }
}

#[cfg(test)]
mod position_tests {
    use super::{LineIndex, Span, byte_to_lsp_position, lsp_position_to_byte};
    use lsp_types::Position;

    // `😀` is U+1F600: 4 UTF-8 bytes, 1 scalar, 2 UTF-16 code units.
    const EMOJI: &str = "😀";

    #[test]
    fn span_containment_uses_half_open_offsets() {
        let span = Span { start: 3, end: 7 };
        assert!(span.contains(span));
        assert!(span.contains(Span { start: 7, end: 7 }));
        assert!(!span.contains(Span { start: 2, end: 7 }));
        assert!(!span.contains(Span { start: 3, end: 8 }));
        assert!(span.contains_offset(3));
        assert!(span.contains_offset(6));
        assert!(!span.contains_offset(7));
        assert!(!Span { start: 3, end: 3 }.contains_offset(3));
    }

    #[test]
    fn line_index_matches_scanning_at_every_offset() {
        for src in ["", "\n", "a😀b\n\nc\r\né😀\n", "x\n😀"] {
            let lines = LineIndex::new(src);
            for offset in 0..=src.len() + 2 {
                assert_eq!(
                    lines.lsp_position(offset),
                    byte_to_lsp_position(src, offset),
                    "{src:?} at {offset}"
                );
            }
        }
    }

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
