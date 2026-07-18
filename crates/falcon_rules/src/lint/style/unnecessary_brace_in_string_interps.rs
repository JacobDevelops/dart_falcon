//! Flags `${x}` string interpolations whose braces can be dropped.
//!
//! When the interpolated expression is a single plain identifier and the character
//! immediately after the closing `}` cannot extend that identifier, `${name}` is
//! equivalent to the leaner `$name`. The braces are still required for anything more
//! than a bare identifier (field access, calls, operators) or when the following
//! character would otherwise merge into the name, so those cases are left alone. Raw
//! strings and escaped `\$` sequences are skipped, and a `$` right after the `}` is
//! treated as extending the name so the braces are kept.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_program};

pub struct UnnecessaryBraceInStringInterps;

impl Rule for UnnecessaryBraceInStringInterps {
    fn name(&self) -> &'static str {
        "unnecessary-brace-in-string-interps"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct StrLit<'a> {
    is_raw: bool,
    content: &'a str,
    content_offset: usize,
}

fn parse_str_lit(raw: &str) -> Option<StrLit<'_>> {
    let is_raw = raw.as_bytes().first() == Some(&b'r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let dlen = if rest.starts_with("'''") || rest.starts_with("\"\"\"") {
        3
    } else if rest.starts_with('\'') || rest.starts_with('"') {
        1
    } else {
        return None;
    };
    let closing = &rest[..dlen];
    if rest.len() < 2 * dlen || !rest[dlen..].ends_with(closing) {
        return None;
    }
    let content_offset = prefix + dlen;
    Some(StrLit {
        is_raw,
        content: &raw[content_offset..raw.len() - dlen],
        content_offset,
    })
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_body(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// A `$` after the `}` starts another interpolation rather than extending the identifier,
/// but keeping the braces there is harmless — so we treat it as "extends" and stay silent.
fn extends_identifier(c: u8) -> bool {
    is_ident_body(c) || c == b'$'
}

/// A single Dart identifier without `$`, so removing the braces is unambiguous.
fn is_simple_identifier(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && is_ident_start(b[0]) && b.iter().all(|&c| is_ident_body(c))
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_string_lit(&mut self, node: &StringLitNode) {
        let Some(lit) = parse_str_lit(&node.raw) else {
            return;
        };
        if lit.is_raw {
            return;
        }
        let bytes = lit.content.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2, // skip escaped char (e.g. `\$`)
                b'$' if i + 1 < bytes.len() && bytes[i + 1] == b'{' => {
                    let Some(close) = matching_brace(bytes, i + 1) else {
                        break;
                    };
                    let inner = &lit.content[i + 2..close];
                    let extends = bytes
                        .get(close + 1)
                        .is_some_and(|&next| extends_identifier(next));
                    if is_simple_identifier(inner) && !extends {
                        let start = node.span.start + lit.content_offset + i;
                        self.diags.push(Diagnostic::new(
                            "unnecessary-brace-in-string-interps",
                            Severity::Warning,
                            "Unnecessary braces in string interpolation; use `$name` instead.",
                            self.file.clone(),
                            DiagSpan {
                                start,
                                end: node.span.start + lit.content_offset + close + 1,
                            },
                        ));
                    }
                    i = close + 1;
                }
                _ => i += 1,
            }
        }
    }
}

/// Given the index of an opening `{`, returns the index of its matching `}`.
fn matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
