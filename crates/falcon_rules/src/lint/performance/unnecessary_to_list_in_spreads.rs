//! Flags a `.toList()` call on the operand of a spread, e.g. `...x.toList()`.
//!
//! A spread element accepts any iterable, so converting to a list first
//! allocates a throwaway `List` for no reason — spread the iterable directly.
//! The rule matches a no-argument `.toList()` call spread into a list, set, or
//! map literal, including null-aware spreads (`...?x.toList()`).

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UnnecessaryToListInSpreads;

impl Rule for UnnecessaryToListInSpreads {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for elem in elements {
                    if let CollectionElement::Spread { expr, .. } = elem
                        && let Some(span) = to_list_call_span(expr)
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "unnecessary-to-list-in-spreads",
                            "Unnecessary 'toList()' in a spread.",
                            *span,
                        ));
                    }
                }
            }
            Expr::Map { elements, .. } => {
                for elem in elements {
                    if let MapElement::Spread { expr, .. } = elem
                        && let Some(span) = to_list_call_span(expr)
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "unnecessary-to-list-in-spreads",
                            "Unnecessary 'toList()' in a spread.",
                            *span,
                        ));
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
    } = expr.unparenthesized()
        && args.positional.is_empty()
        && args.named.is_empty()
        && let Expr::Field { field, .. } = callee.unparenthesized()
        && field.name == "toList"
    {
        return Some(span);
    }
    None
}
