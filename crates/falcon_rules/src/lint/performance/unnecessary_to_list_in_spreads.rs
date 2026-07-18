//! Flags a `.toList()` call on the operand of a spread, e.g. `...x.toList()`.
//!
//! A spread element accepts any iterable, so converting to a list first
//! allocates a throwaway `List` for no reason — spread the iterable directly.
//! The rule matches a no-argument `.toList()` call spread into a list, set, or
//! map literal, including null-aware spreads (`...?x.toList()`).

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UnnecessaryToListInSpreads;

impl Rule for UnnecessaryToListInSpreads {
    fn name(&self) -> &'static str {
        "unnecessary-to-list-in-spreads"
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

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "unnecessary-to-list-in-spreads",
            Severity::Warning,
            "Unnecessary 'toList()' in a spread.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for elem in elements {
                    if let CollectionElement::Spread { expr, .. } = elem
                        && let Some(span) = to_list_call_span(expr)
                    {
                        self.push(span);
                    }
                }
            }
            Expr::Map { elements, .. } => {
                for elem in elements {
                    if let MapElement::Spread { expr, .. } = elem
                        && let Some(span) = to_list_call_span(expr)
                    {
                        self.push(span);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

/// Span of a no-argument `.toList()` call, or `None` for anything else.
fn to_list_call_span(expr: &Expr) -> Option<&Span> {
    if let Expr::Call {
        callee, args, span, ..
    } = expr
        && args.positional.is_empty()
        && args.named.is_empty()
        && let Expr::Field { field, .. } = &**callee
        && field.name == "toList"
    {
        return Some(span);
    }
    None
}
