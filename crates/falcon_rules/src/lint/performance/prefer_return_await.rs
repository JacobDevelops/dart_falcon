//! Await returned SDK Futures inside a try body protected by a catch clause.
//!
//! Without awaiting, asynchronous failures bypass the local catch clauses.
//! Function-body boundaries prevent an outer try from affecting nested closures.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::function_bodies::FunctionBodies;

#[derive(Default)]
pub struct PreferReturnAwait;

impl Rule for PreferReturnAwait {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            bodies: FunctionBodies::new(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, value: Expr<'a>, state: &SemanticState<'a>) {
        let value = value.unparenthesized();
        let Some(body) = self.bodies.enclosing(&value.span()) else {
            return;
        };
        if matches!(value, Expr::Await(_))
            || body.async_marker != AsyncMarker::Async
            || !protected_by_catch(value.syntax(), body.span.start)
        {
            return;
        }
        let ty = state.infer(value);
        if ty.interface("dart:async", "Future") && !ty.nullable() {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-return-await",
                "Returning this Future without awaiting lets asynchronous failures bypass the catch block. Return 'await' on the Future instead.",
                value.span(),
            ));
        }
    }
}

fn protected_by_catch(mut node: SyntaxNode<'_>, body_start: usize) -> bool {
    while let Some(parent) = node.parent() {
        if parent.span().start < body_start {
            return false;
        }
        if let Some(Stmt::TryCatch(statement)) = Stmt::cast(parent)
            && statement.body().syntax().id() == node.id()
            && !statement.catches().is_empty()
        {
            return true;
        }
        node = parent;
    }
    false
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Return(statement) = node
            && let Some(value) = statement.value()
        {
            self.check(value, state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
