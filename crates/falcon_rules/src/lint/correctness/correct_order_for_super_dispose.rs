//! Require `super.dispose()` to be the last call in `dispose`.
//!
//! Flags a `super.dispose()` invocation that is not the final statement of a
//! `dispose` method. The base `State.dispose` tears down the framework's own
//! bookkeeping for the object, after which touching the widget's fields is
//! unsafe, so any cleanup your subclass performs must happen first. A
//! `super.dispose()` in the middle of the method leaves the remaining cleanup
//! running against a half-disposed object. Move the `super.dispose()` call to
//! the end.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CorrectOrderForSuperDispose;

impl Rule for CorrectOrderForSuperDispose {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in crate::lint::declarations::members_of(decl).unwrap_or_default() {
        scan_member(member, diags, ctx);
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let ClassMember::Method(m) = member
        && m.name.name == "dispose"
        && let Some(FunctionBody::Block(block)) = &m.body
    {
        check_dispose_method(block, diags, ctx);
    }
}

fn check_dispose_method(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Find super.dispose() calls in top-level statements
    let last_index = block.stmts.len().saturating_sub(1);

    for (idx, stmt) in block.stmts.iter().enumerate() {
        if let Stmt::Expr(ExprStmt { expr, .. }) = stmt
            && is_super_dispose_call(expr)
            && idx != last_index
        {
            // super.dispose() is not the last statement
            diags.push(ctx.diagnostic(
                "correct-order-for-super-dispose",
                "super.dispose() should be called last in the dispose method.",
                *expr.span(),
            ));
        }
    }
}

fn is_super_dispose_call(expr: &Expr) -> bool {
    if let Expr::Call { callee, .. } = expr
        && let Expr::Field { object, field, .. } = callee.as_ref()
        && matches!(object.as_ref(), Expr::Super { .. })
        && field.name == "dispose"
    {
        return true;
    }
    false
}
