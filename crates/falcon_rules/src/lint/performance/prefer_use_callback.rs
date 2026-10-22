//! Prefer `useCallback` for synchronous memoized function literals.
//! Only resolved flutter_hooks calls with a zero-argument, single-expression
//! builder are checked; asynchronous and side-effectful builders are preserved.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferUseCallback;

impl Rule for PreferUseCallback {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
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
            self.ctx.identities().is_some_and(|identities| {
                identities.is_package_declaration(&identity, "flutter_hooks", "useMemoized")
            })
        }) {
            return;
        }
        let Some(Expr::FuncExpr(builder)) =
            call.args().positional().next().map(Expr::unparenthesized)
        else {
            return;
        };
        if builder.async_marker() != AsyncMarker::Sync || !builder.params().params().is_empty() {
            return;
        }
        let value = match builder.body() {
            FunctionBody::Arrow(body) => Some(body.expr()),
            FunctionBody::Block(body) if body.stmts().len() == 1 => match body.stmts().first() {
                Some(Stmt::Return(statement)) => statement.value(),
                _ => None,
            },
            _ => None,
        };
        if value.is_some_and(|value| matches!(value.unparenthesized(), Expr::FuncExpr(_))) {
            self.diagnostics.push(self.ctx.diagnostic("Prefer `useCallback` instead of `useMemoized` for function expressions.", call.callee().span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
