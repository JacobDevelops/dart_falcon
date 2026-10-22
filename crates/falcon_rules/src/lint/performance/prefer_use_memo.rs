//! Prefer memoizing freshly allocated collection values used as hook keys.
//! Flutter Hooks compares individual keys with ==; Dart collection literals use
//! identity equality, so allocating a key again invalidates the memo every build.
//! Only direct non-const collection literals in an explicit dependency list of
//! resolved useEffect/useMemoized/useCallback calls are checked. This deliberately
//! does not guess whether arbitrary build computations are expensive or stable.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferUseMemo;

impl Rule for PreferUseMemo {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else { return };
        if !crate::lint::flutter_hooks::call_identity(expression, state).is_some_and(|identity| {
            ["useEffect", "useMemoized", "useCallback"]
                .iter()
                .any(|name| {
                    self.ctx.identities().is_some_and(|identities| {
                        identities.is_package_declaration(&identity, "flutter_hooks", name)
                    })
                })
        }) {
            return;
        }
        let Some(Expr::List(keys)) = call.args().positional().nth(1).map(Expr::unparenthesized)
        else {
            return;
        };
        if keys.is_const() {
            return;
        }
        for element in keys.elements() {
            let CollectionElement::Expr(value) = element else {
                continue;
            };
            let allocated = match value.unparenthesized() {
                Expr::List(list) => !list.is_const(),
                Expr::Set(set) => !set.is_const(),
                Expr::Map(map) => !map.is_const(),
                _ => false,
            };
            if allocated {
                self.diagnostics.push(self.ctx.diagnostic("Memoize this collection with `useMemoized` before using it as a hook dependency key.", value.span()));
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
