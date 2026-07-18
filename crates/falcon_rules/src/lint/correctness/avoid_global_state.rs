//! Disallow mutable global and static state.
//!
//! Flags top-level variables and `static` fields that are mutable — anything not
//! declared `const` or plain `final`, including `late final`, which can still be
//! assigned once at an arbitrary point. Mutable global state is reachable and
//! writable from anywhere, which makes data flow hard to trace, introduces
//! initialization-order and concurrency hazards, and lets tests contaminate one
//! another through leftover shared state. Prefer `const` or `final`
//! declarations, and thread mutable state explicitly through constructors or
//! parameters rather than hanging it off globals.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidGlobalState;

impl Rule for AvoidGlobalState {
    fn name(&self) -> &'static str {
        "avoid-global-state"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Variable(var)
                    if is_mutable(var.is_const, var.is_final, var.is_late) =>
                {
                    diags.push(make_diag(ctx, &var.span));
                }
                TopLevelDecl::Class(class) => check_static_fields(&class.members, &mut diags, ctx),
                TopLevelDecl::Mixin(mixin) => check_static_fields(&mixin.members, &mut diags, ctx),
                TopLevelDecl::MixinClass(mc) => check_static_fields(&mc.members, &mut diags, ctx),
                _ => {}
            }
        }
        diags
    }
}

fn is_mutable(is_const: bool, is_final: bool, is_late: bool) -> bool {
    !is_const && (!is_final || is_late)
}

fn check_static_fields(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in members {
        if let ClassMember::Field(field) = member
            && field.is_static
            && is_mutable(field.is_const, field.is_final, field.is_late)
        {
            diags.push(make_diag(ctx, &field.span));
        }
    }
}

fn make_diag(ctx: &AnalyzeContext, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "avoid-global-state",
        Severity::Warning,
        "Avoid mutable global state — use const or final declarations instead",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}
