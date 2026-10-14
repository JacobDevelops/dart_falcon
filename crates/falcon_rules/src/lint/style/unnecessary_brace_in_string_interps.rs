//! Flags `${x}` string interpolations whose braces can be dropped.
//!
//! When the interpolated expression is a single plain identifier and the character
//! immediately after the closing `}` cannot extend that identifier, `${name}` is
//! equivalent to the leaner `$name`. The braces are still required for anything more
//! than a bare identifier (field access, calls, operators) or when the following
//! character would otherwise merge into the name, so those cases are left alone. Raw
//! strings and escaped `\$` sequences are skipped, and a `$` right after the `}` is
//! treated as extending the name so the braces are kept.

use crate::lint::string_lit::{
    interpolated_literals, is_ident_body, is_simple_identifier, matching_brace,
};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct UnnecessaryBraceInStringInterps;

impl Rule for UnnecessaryBraceInStringInterps {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// A `$` after the `}` starts another interpolation rather than extending the identifier,
/// but keeping the braces there is harmless — so we treat it as "extends" and stay silent.
fn extends_identifier(c: u8) -> bool {
    is_ident_body(c) || c == b'$'
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in interpolated_literals(node) {
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
                            let start = span.start + lit.content_offset + i;
                            self.diags.push(self.ctx.diagnostic(
                                "unnecessary-brace-in-string-interps",
                                "Unnecessary braces in string interpolation; use `$name` instead.",
                                DiagSpan {
                                    start,
                                    end: span.start + lit.content_offset + close + 1,
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
}
