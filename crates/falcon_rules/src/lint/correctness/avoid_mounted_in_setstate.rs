//! Check mounted before invoking Flutter State.setState, never inside its callback.
//! A check inside the callback cannot prevent setState's unmounted-State assertion.
//! Deferred nested closures and local functions have independent mounted checks.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::flutter_state_rules::is_setstate;
use super::use_build_context_synchronously::{mounted_kind, mounted_receiver_kind};

#[derive(Default)]
pub struct AvoidMountedInSetstate;

impl Rule for AvoidMountedInSetstate {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            callbacks: Vec::new(),
            closures: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    callbacks: Vec<Span>,
    closures: Vec<Span>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>, _state: &SemanticState<'a>) {
        if let Stmt::LocalFunc(_) = statement {
            self.closures.push(statement.span());
        }
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::FuncExpr(function) = expression {
            self.closures.push(function.span());
        }
        if let Expr::Call(call) = expression
            && is_setstate(call, state)
            && let Some(Expr::FuncExpr(callback)) =
                call.args().positional().next().map(Expr::unparenthesized)
        {
            self.callbacks.push(callback.span());
        }
        let scope = self.closures.iter().rev().find(|scope| {
            scope.start <= expression.span().start && expression.span().end <= scope.end
        });
        if !scope.is_some_and(|scope| self.callbacks.contains(scope)) {
            return;
        }
        let mounted = match expression {
            Expr::Ident(name)
                if name.name() == "mounted" && !state.environment.is_bound("mounted") =>
            {
                state
                    .enclosing_type
                    .as_ref()
                    .and_then(|ty| mounted_kind(ty, state))
                    .is_some()
            }
            Expr::Field(field) if field.field().name() == "mounted" => {
                mounted_receiver_kind(field.object(), state).is_some()
            }
            _ => false,
        };
        if mounted {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-mounted-in-setstate",
                "Check mounted before calling setState, not inside its callback.",
                expression.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
