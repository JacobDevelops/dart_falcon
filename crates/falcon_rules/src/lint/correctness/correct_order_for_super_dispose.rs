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
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct CorrectOrderForSuperDispose;

impl Rule for CorrectOrderForSuperDispose {
    fn name(&self) -> &'static str {
        "correct-order-for-super-dispose"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Extension(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        _ => {}
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
            diags.push(Diagnostic::new(
                "correct-order-for-super-dispose",
                Severity::Warning,
                "super.dispose() should be called last in the dispose method.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: expr.span().start,
                    end: expr.span().end,
                },
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
