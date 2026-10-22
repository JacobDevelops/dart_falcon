//! Flags a trailing `if` that wraps multiple statements in a void function.
//!
//! Invert the condition and return early to remove a level of indentation.
//! Only a function body's final statement is considered, with no `else`, a
//! plain expression condition, and a block with multiple statements or a nested
//! `if`. Explicitly void functions, methods, operators and local functions,
//! setters, and constructors are checked. Non-void and inferred-return functions,
//! closures, generators, pattern conditions, and single-statement guard clauses
//! are left alone: a bare return must not change a value-returning contract.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferEarlyReturn;

impl Rule for PreferEarlyReturn {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, body: Option<FunctionBody<'_>>) {
        if let Some(FunctionBody::Block(block)) = body
            && let Some(Stmt::If(condition)) = block.stmts().last()
            && condition.else_branch().is_none()
            && matches!(condition.condition(), IfCondition::Expr(_))
            && let Stmt::Block(branch) = condition.then_branch()
            && (branch.stmts().len() >= 2 || matches!(branch.stmts().first(), Some(Stmt::If(_))))
        {
            self.diags.push(self.ctx.diagnostic("Invert this condition and return early to reduce nesting.", condition.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if (matches!(node.return_type(), Some(DartType::Void(_)))
            || matches!(node.kind(), FunctionKind::Setter(_)))
            && matches!(node.async_marker(), AsyncMarker::Sync | AsyncMarker::Async)
        {
            self.check(node.body());
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        match node {
            ClassMember::Method(method)
                if matches!(method.return_type(), Some(DartType::Void(_)))
                    && matches!(method.async_marker(), AsyncMarker::Sync | AsyncMarker::Async) =>
            {
                self.check(method.body());
            }
            ClassMember::Operator(operator)
                if matches!(operator.return_type(), Some(DartType::Void(_)))
                    && matches!(operator.async_marker(), AsyncMarker::Sync | AsyncMarker::Async) =>
            {
                self.check(operator.body());
            }
            ClassMember::Setter(setter) => self.check(setter.body()),
            ClassMember::Constructor(constructor) => self.check(constructor.body()),
            ClassMember::PrimaryConstructorBody(constructor) => self.check(constructor.body()),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node
            && matches!(function.return_type(), Some(DartType::Void(_)))
            && matches!(function.async_marker(), AsyncMarker::Sync | AsyncMarker::Async)
        {
            self.check(Some(function.body()));
        }
        StructuralTraversal::Continue
    }
}
}
