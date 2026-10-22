//! Flags an `async` function whose body is a single `return await ...;`.
//!
//! When a function does nothing with the awaited value but return it, the
//! `async`/`await` round-trip is redundant: returning the `Future` directly is
//! equivalent and skips an extra microtask hop and stack frame. The rule fires
//! on a top-level function or method marked `async` whose block body is exactly
//! one statement of the form `return await <expr>;`. Local functions and
//! anonymous closures are not checked.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidRedundantAsync;

impl Rule for AvoidRedundantAsync {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, AsyncDeclarations, ctx, diags)
    }
}

struct AsyncDeclarations<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl<'a, 'ctx> StructuralRuleVisitor<'ctx> for AsyncDeclarations<'a, 'ctx> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, _node: Expr<'ctx>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'ctx>) -> StructuralTraversal {
        if matches!(node, TopLevelDecl::Variable(_)) {
            StructuralTraversal::SkipChildren
        } else {
            StructuralTraversal::Continue
        }
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'ctx>) -> StructuralTraversal {
        if let Some(diag) = check_function_async(node, self.ctx) {
            self.diags.push(diag);
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_class_member(&mut self, node: ClassMember<'ctx>) -> StructuralTraversal {
        if let ClassMember::Method(method) = node
            && let Some(diag) = check_method_async(method, self.ctx)
        {
            self.diags.push(diag);
        }
        StructuralTraversal::SkipChildren
    }
}
}

fn check_function_async(func_decl: FunctionDecl<'_>, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !func_decl.async_marker().is_async() {
        return None;
    }

    if is_redundant_async_body(func_decl.body()) {
        return Some(ctx.diagnostic("Remove `async` and `await` to return the `Future` directly.", func_decl.name().span()));
    }

    None
}

fn check_method_async(method_decl: MethodDecl<'_>, ctx: &AnalyzeContext) -> Option<Diagnostic> {
    if !method_decl.async_marker().is_async() {
        return None;
    }

    if is_redundant_async_body(method_decl.body()) {
        return Some(ctx.diagnostic("Remove `async` and `await` to return the `Future` directly.", method_decl.name().span()));
    }

    None
}

fn is_redundant_async_body(body: Option<FunctionBody<'_>>) -> bool {
    match body {
        Some(FunctionBody::Block(block)) => {
            block.stmts().len() == 1
                && matches!(block.stmts().next(), Some(Stmt::Return(returned))
                    if returned.value().is_some_and(|value| matches!(value.unparenthesized(), Expr::Await(_))))
        }
        _ => false,
    }
}
