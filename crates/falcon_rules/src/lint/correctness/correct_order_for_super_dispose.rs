//! Require `super.dispose()` to be the last call in `dispose`.
//!
//! Flags a `super.dispose()` invocation that is not the final statement of a
//! `dispose` method. The base `State.dispose` tears down the framework's own
//! bookkeeping for the object, after which touching the widget's fields is
//! unsafe, so any cleanup your subclass performs must happen first. A
//! `super.dispose()` in the middle of the method leaves the remaining cleanup
//! running against a half-disposed object. Move the `super.dispose()` call to
//! the end.
//! Checks direct statements of method block bodies, not nested blocks,
//! callbacks, or other declarations named `dispose`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct CorrectOrderForSuperDispose;

impl Rule for CorrectOrderForSuperDispose {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        if node.name().name() == "dispose"
            && let Some(FunctionBody::Block(block)) = node.body()
        {
            check_dispose_method(block, &mut self.diags, self.ctx);
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, _node: Stmt<'a>) -> StructuralTraversal { StructuralTraversal::SkipChildren }

    fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal { StructuralTraversal::SkipChildren }

    fn visit_dart_type(&mut self, _node: DartType<'a>) -> StructuralTraversal { StructuralTraversal::SkipChildren }
}
}

fn check_dispose_method(block: Block<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let last_index = block.stmts().len().saturating_sub(1);

    for (idx, stmt) in block.stmts().enumerate() {
        if let Stmt::Expr(stmt) = stmt
            && is_super_dispose_call(stmt.expr())
            && idx != last_index
        {
            diags.push(ctx.diagnostic("Call `super.dispose()` last in the `dispose` method.", stmt.expr().span()));
        }
    }
}

fn is_super_dispose_call(expr: Expr<'_>) -> bool {
    if let Expr::Call(call) = expr.unparenthesized()
        && let Expr::Field(field) = call.callee().unparenthesized()
        && matches!(field.object().unparenthesized(), Expr::Super(_))
        && field.field().name() == "dispose"
    {
        return true;
    }
    false
}
