//! Flags source lines longer than 80 characters.
//!
//! Long lines are harder to read and to review side by side. Line length is
//! counted in UTF-16 code units, excluding the line break, and the diagnostic
//! starts at the 81st character, where the line needs to wrap.
//!
//! Lines that cannot reasonably be wrapped are exempt:
//!
//! - every line covered by a multiline (triple-quoted) string literal;
//! - a line where a single-line string literal starts whose text contains `/`
//!   or `\` (a URI or path);
//! - a comment line whose text contains `/` or `\`, and a `// ignore:`
//!   comment line.
//!
//! ```dart
//! // A comment that runs on and on and on and on and on and on and on and on.
//! //                                                    flagged ^ past column 80
//! // See https://github.com/dart-lang/sdk/blob/main/pkg/linter/doc/very/long.md
//! //                                  fine: the comment contains a URI ^
//! ```
//!
//! Adopted from the official `lines_longer_than_80_chars` lint.

use crate::lint::string_lit::{decoded_text, literal_parts};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::Visitor;

const MAX_LENGTH: usize = 80;

#[derive(Default)]
pub struct LinesLongerThan80Chars;

impl Rule for LinesLongerThan80Chars {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let source = ctx.source;
        let lines = Lines::new(source);
        let long_lines: Vec<(usize, usize)> = (0..lines.starts.len())
            .filter_map(|index| {
                let (start, end) = lines.content_range(index);
                let wrap = byte_offset_after_utf16_units(&source[start..end], MAX_LENGTH)?;
                Some((index, start + wrap))
            })
            .collect();
        if long_lines.is_empty() {
            return Vec::new();
        }

        let mut allowed = vec![false; lines.starts.len()];
        let mut strings = AllowedStrings {
            lines: &lines,
            allowed: &mut allowed,
        };
        strings.visit_program(program);
        allow_comment_lines(source, &lines, &mut allowed);

        long_lines
            .into_iter()
            .filter(|&(index, _)| !allowed[index])
            .map(|(index, wrap)| {
                let (_, end) = lines.content_range(index);
                ctx.diagnostic(
                    "lines-longer-than-80-chars",
                    "The line length exceeds the 80-character limit.",
                    DiagSpan { start: wrap, end },
                )
            })
            .collect()
    }
}

/// Line starts of a source split on `\n`, `\r\n`, and `\r`.
struct Lines<'a> {
    source: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        let mut starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    i += 2;
                    starts.push(i);
                }
                b'\r' | b'\n' => {
                    i += 1;
                    starts.push(i);
                }
                _ => i += 1,
            }
        }
        Lines { source, starts }
    }

    /// The byte range of a line, excluding its line break.
    fn content_range(&self, index: usize) -> (usize, usize) {
        let start = self.starts[index];
        let end = self
            .starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.source.len());
        let text = &self.source[start..end];
        let trimmed = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix(['\n', '\r']))
            .unwrap_or(text);
        (start, start + trimmed.len())
    }

    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset) - 1
    }
}

/// The byte offset just past the first `units` UTF-16 code units of `text`,
/// or `None` when `text` is not longer than that.
fn byte_offset_after_utf16_units(text: &str, units: usize) -> Option<usize> {
    let mut seen = 0;
    for (offset, c) in text.char_indices() {
        if seen >= units {
            return Some(offset);
        }
        seen += c.len_utf16();
    }
    None
}

fn looks_like_uri_or_path(text: &str) -> bool {
    text.contains(['/', '\\'])
}

struct AllowedStrings<'a, 'b> {
    lines: &'a Lines<'a>,
    allowed: &'b mut [bool],
}

impl Visitor for AllowedStrings<'_, '_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in node.segments() {
            let first = self.lines.line_of(span.start);
            if lit.triple {
                let last = self.lines.line_of(span.end);
                self.allowed[first..=last].fill(true);
            } else if looks_like_uri_or_path(&decoded_text(&lit, &literal_parts(&lit))) {
                self.allowed[first] = true;
            }
        }
    }
}

fn allow_comment_lines(source: &str, lines: &Lines<'_>, allowed: &mut [bool]) {
    for token in Lexer::new(source).tokenize() {
        if !matches!(
            token.kind,
            TokenKind::LineComment | TokenKind::DocComment | TokenKind::BlockComment
        ) {
            continue;
        }
        let lexeme = token.text(source);
        let first = lines.line_of(token.offset);
        if let Some(text) = lexeme.strip_prefix("///") {
            if looks_like_uri_or_path(text) {
                allowed[first] = true;
            }
        } else if let Some(text) = lexeme.strip_prefix("//") {
            if text.trim_start().starts_with("ignore:") || looks_like_uri_or_path(text) {
                allowed[first] = true;
            }
        } else if let Some(text) = lexeme.strip_prefix("/*") {
            let text = text.strip_suffix("*/").unwrap_or(text);
            let comment_lines = Lines::new(text);
            for index in 0..comment_lines.starts.len() {
                let (start, end) = comment_lines.content_range(index);
                if looks_like_uri_or_path(&text[start..end])
                    && let Some(line) = allowed.get_mut(first + index)
                {
                    *line = true;
                }
            }
        }
    }
}
