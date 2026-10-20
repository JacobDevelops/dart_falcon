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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseRethrowWhenPossible;

impl Rule for UseRethrowWhenPossible {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

fn is_target(expr: Expr<'_>, target: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(id) if id.name() == target)
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_catch_clause(&mut self, node: CatchClause<'a>) -> StructuralTraversal {
        self.targets.push(self.target.take());
        self.target = node.exception_var()
            .and_then(|var| self.ctx.tree().tokens_in(self.ctx.source(), var.span()).next())
            .map(|token| token.text());
        StructuralTraversal::Continue
    }

    fn exit_catch_clause(&mut self, _node: CatchClause<'a>) {
        self.target = self.targets.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if matches!(node, Stmt::TryCatch(_) | Stmt::LocalFunc(_)) {
            self.targets.push(self.target.take());
        }
        if let Stmt::Throw(throw) = node
            && self.target.as_ref().is_some_and(|target| is_target(throw.value(), target))
        {
            self.report(throw.span());
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::TryCatch(_) | Stmt::LocalFunc(_)) {
            self.target = self.targets.pop().unwrap();
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr(_)) {
            self.targets.push(self.target.take());
        }
        if let Expr::Throw(throw) = node
            && self.target.as_ref().is_some_and(|target| is_target(throw.expr(), target))
        {
            self.report(throw.span());
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.target = self.targets.pop().unwrap();
        }
    }
}
}
