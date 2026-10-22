//! Flags a `.toList()` call on the operand of a spread, e.g. `...x.toList()`.
//!
//! A spread element accepts any iterable, so converting to a list first
//! allocates a throwaway `List` for no reason — spread the iterable directly.
//! The rule matches a no-argument `.toList()` call spread into a list, set, or
//! map literal, including null-aware spreads (`...?x.toList()`).

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryToListInSpreads;

impl Rule for UnnecessaryToListInSpreads {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::List(list) => self.check_elements(list.elements()),
            Expr::Set(set) => self.check_elements(set.elements()),
            Expr::Map(map) => {
                for elem in map.elements() {
                    if let MapElement::Spread(spread) = elem
                        && let Some(span) = to_list_call_span(spread.expr())
                    {
                        self.report(span);
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

impl Collector<'_> {
    fn check_elements(&mut self, elements: AstChildren<'_, CollectionElement<'_>>) {
        for elem in elements {
            if let CollectionElement::Spread(spread) = elem
                && let Some(span) = to_list_call_span(spread.expr())
            {
                self.report(span);
            }
        }
    }

    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic("Remove the unnecessary `toList()` call from this spread.", span));
    }
}

/// Span of a no-argument `.toList()` call, or `None` for anything else.
fn to_list_call_span(expr: Expr<'_>) -> Option<Span> {
    if let Expr::Call(call) = expr.unparenthesized()
        && call.args().positional().is_empty()
        && call.args().named().is_empty()
        && let Expr::Field(field) = call.callee().unparenthesized()
        && field.field().name() == "toList"
    {
        return Some(call.span());
    }
    None
}
