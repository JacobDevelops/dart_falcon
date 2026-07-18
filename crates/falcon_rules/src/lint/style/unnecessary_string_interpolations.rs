//! Flags a string whose entire content is a single interpolation. Ported from package:lints
//! `unnecessary_string_interpolations`. `'$x'` / `'${x}'` where the whole string is just one
//! interpolated expression is equivalent to writing the expression directly.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_program};

pub struct UnnecessaryStringInterpolations;

impl Rule for UnnecessaryStringInterpolations {
    fn name(&self) -> &'static str {
        "unnecessary-string-interpolations"
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
    Some(StrLit {
        is_raw,
        content: &raw[prefix + dlen..raw.len() - dlen],
    })
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

// `$` is excluded so that `'$a$b'` (two interpolations) is not mistaken for one identifier.
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// True when `content` is exactly one interpolation and nothing else: `${...}` spanning the
/// whole content, or `$identifier` spanning the whole content.
fn is_whole_interpolation(content: &str) -> bool {
    let bytes = content.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'$' {
        return false;
    }
    if bytes[1] == b'{' {
        // `${...}` must close exactly at the final byte, with a non-empty expression.
        let mut depth = 0usize;
        let mut i = 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 1,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i == bytes.len() - 1 && i > 2;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    } else {
        // `$identifier` covering the whole content.
        is_ident_start(bytes[1]) && bytes[1..].iter().all(|&c| is_ident_continue(c))
    }
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
        if lit.is_raw || !is_whole_interpolation(lit.content) {
            return;
        }
        self.diags.push(Diagnostic::new(
            "unnecessary-string-interpolations",
            Severity::Warning,
            "Unnecessary string interpolation; use the expression directly.",
            self.file.clone(),
            DiagSpan {
                start: node.span.start,
                end: node.span.end,
            },
        ));
    }
}
