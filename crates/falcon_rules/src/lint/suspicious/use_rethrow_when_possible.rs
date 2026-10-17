//! Flags `throw e` inside a `catch (e)` by matching the exception parameter's name.
//!
//! Re-throwing the caught object with `throw e` starts a fresh stack trace from
//! the catch site, discarding the original point of failure and making the error
//! far harder to trace. Use `rethrow`, which propagates the same exception with
//! its trace intact. Matching is syntactic: the thrown identifier must have the
//! exception parameter's spelling; binding identity is not resolved. Throws
//! inside a nested closure or a nested `try`/`catch` are skipped.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseRethrowWhenPossible;

impl Rule for UseRethrowWhenPossible {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            target: None,
            targets: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    target: Option<&'a str>,
    targets: Vec<Option<&'a str>>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "use-rethrow-when-possible",
            "Use `rethrow` to re-throw the caught exception and preserve its stack trace.",
            span,
        ));
    }
}

fn is_target(expr: &Expr, target: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(id) if id.name == target)
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_catch_clause(&mut self, node: &CatchClause) -> StructuralTraversal {
        self.targets.push(self.target.take());
        self.target = node.exception_var.as_ref()
            .and_then(|var| self.ctx.syntax.tokens_in(var.span).next())
            .map(|token| token.text(self.ctx.source));
        StructuralTraversal::Continue
    }

    fn exit_catch_clause(&mut self, _node: &CatchClause) {
        self.target = self.targets.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if matches!(node, Stmt::TryCatch(_) | Stmt::LocalFunc(_)) {
            self.targets.push(self.target.take());
        }
        if let Stmt::Throw(throw) = node
            && self.target.as_ref().is_some_and(|target| is_target(&throw.value, target))
        {
            self.report(throw.span);
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: &Stmt) {
        if matches!(node, Stmt::TryCatch(_) | Stmt::LocalFunc(_)) {
            self.target = self.targets.pop().unwrap();
        }
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.targets.push(self.target.take());
        }
        if let Expr::Throw { expr, span } = node
            && self.target.as_ref().is_some_and(|target| is_target(expr, target))
        {
            self.report(*span);
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.target = self.targets.pop().unwrap();
        }
    }
}
}
