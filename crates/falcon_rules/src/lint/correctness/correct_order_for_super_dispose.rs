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
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CorrectOrderForSuperDispose;

impl Rule for CorrectOrderForSuperDispose {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
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
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        if node.name.name == "dispose"
            && let Some(FunctionBody::Block(block)) = &node.body
        {
            check_dispose_method(block, &mut self.diags, self.ctx);
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, _node: &Stmt) -> StructuralTraversal { StructuralTraversal::SkipChildren }

    fn visit_expr(&mut self, _node: &Expr) -> StructuralTraversal { StructuralTraversal::SkipChildren }

    fn visit_dart_type(&mut self, _node: &DartType) -> StructuralTraversal { StructuralTraversal::SkipChildren }
}
}

fn check_dispose_method(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let last_index = block.stmts.len().saturating_sub(1);

    for (idx, stmt) in block.stmts.iter().enumerate() {
        if let Stmt::Expr(ExprStmt { expr, .. }) = stmt
            && is_super_dispose_call(expr)
            && idx != last_index
        {
            diags.push(ctx.diagnostic(
                "correct-order-for-super-dispose",
                "super.dispose() should be called last in the dispose method.",
                *expr.span(),
            ));
        }
    }
}

fn is_super_dispose_call(expr: &Expr) -> bool {
    if let Expr::Call { callee, .. } = expr.unparenthesized()
        && let Expr::Field { object, field, .. } = callee.unparenthesized()
        && matches!(object.unparenthesized(), Expr::Super { .. })
        && field.name == "dispose"
    {
        return true;
    }
    false
}
