//! Flags source lines longer than 80 characters.
//!
//! Long lines are harder to read and to review side by side. Line length is
//! counted in UTF-16 code units, excluding the line break and a leading byte
//! order mark, and the diagnostic starts at the 81st character, where the line
//! needs to wrap.
//!
//! Lines that cannot reasonably be wrapped are exempt:
//!
//! - every line covered by a multiline (triple-quoted) string literal;
//! - a line where a single-line string literal starts whose text contains `/`
//!   or `\` (a URI or path); as in the analyzer, string literals nested in an
//!   interpolation are not considered;
//! - a comment line (including comments inside interpolations) whose text
//!   contains `/` or `\`, and a `// ignore:` comment line.
//!
//! ```dart
//! // A comment that runs on and on and on and on and on and on and on and on.
//! //                                                    flagged ^ past column 80
//! // See https://github.com/dart-lang/sdk/blob/main/pkg/linter/doc/very/long.md
//! //                                  fine: the comment contains a URI ^
//! ```
//!
//! Adopted from the official `lines_longer_than_80_chars` lint.

use crate::lint::string_lit::decode_escapes;
use crate::lint::tokens::comment_tokens;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

const MAX_LENGTH: usize = 80;

#[derive(Default)]
pub struct LinesLongerThan80Chars;

impl Rule for LinesLongerThan80Chars {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
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
            return RulePreparation::Diagnostics(Vec::new());
        }

        let mut allowed = vec![false; lines.starts.len()];
        let mut strings = AllowedStrings {
            lines: &lines,
            allowed: &mut allowed,
            syntax: ctx.syntax,
        };
        strings.visit_program(program);
        allow_comment_lines(ctx, &lines, &mut allowed);

        RulePreparation::Diagnostics(
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
                .collect(),
        )
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
        // The scanner drops a leading byte order mark.
        let bom = if source.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        let mut starts = vec![bom];
        let mut i = bom;
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
        self.starts.partition_point(|&start| start <= offset).max(1) - 1
    }
}

/// The byte offset of the character holding UTF-16 code unit `units` (the
/// first unit past the limit) of `text`, or `None` when `text` is not longer
/// than `units`. A surrogate pair straddling the limit counts.
fn byte_offset_after_utf16_units(text: &str, units: usize) -> Option<usize> {
    let mut seen = 0;
    for (offset, c) in text.char_indices() {
        seen += c.len_utf16();
        if seen > units {
            return Some(offset);
        }
    }
    None
}

fn looks_like_uri_or_path(text: &str) -> bool {
    text.contains(['/', '\\'])
}

struct AllowedStrings<'a, 'b> {
    lines: &'a Lines<'a>,
    allowed: &'b mut [bool],
    syntax: &'a falcon_syntax::SyntaxStore,
}

impl Visitor for AllowedStrings<'_, '_> {
    // Interpolated paths cannot exempt their surrounding line.
    fn visit_string_interpolation(&mut self, _node: &StringInterpolation) {}

    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in node.segments() {
            let first = self.lines.line_of(span.start);
            if lit.triple {
                let last = self.lines.line_of(span.end);
                self.allowed[first..=last].fill(true);
            } else if literal_contains_path(&lit, span.start + lit.content_offset, self.syntax) {
                self.allowed[first] = true;
            }
        }
    }
}

/// Only literal text counts: a path inside an interpolated expression does not.
fn literal_contains_path(
    lit: &StringDelimiter<'_>,
    content_start: usize,
    syntax: &falcon_syntax::SyntaxStore,
) -> bool {
    if lit.is_raw {
        return looks_like_uri_or_path(lit.content);
    }
    let bytes = lit.content.as_bytes();
    let mut text_start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'$' if bytes.get(index + 1) == Some(&b'{') => {
                if looks_like_uri_or_path(&decode_escapes(&lit.content[text_start..index])) {
                    return true;
                }
                let inner = index + 2;
                let Some(close) = syntax.interpolation_end(content_start + inner) else {
                    return false;
                };
                index = close + 1 - content_start;
                text_start = index;
            }
            _ => index += 1,
        }
    }
    looks_like_uri_or_path(&decode_escapes(&lit.content[text_start..]))
}

/// Allow comment lines, including comments inside interpolations.
fn allow_comment_lines(ctx: &AnalyzeContext, lines: &Lines<'_>, allowed: &mut [bool]) {
    let source = ctx.source;
    for token in comment_tokens(ctx) {
        let lexeme = token.text(source);
        let first = lines.line_of(token.offset());
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
            let comment_lines = text.split("\r\n").flat_map(|line| line.split(['\r', '\n']));
            for (index, text) in comment_lines.enumerate() {
                if looks_like_uri_or_path(text)
                    && let Some(line) = allowed.get_mut(first + index)
                {
                    *line = true;
                }
            }
        }
    }
}
