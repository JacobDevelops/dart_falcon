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
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidGlobalState;

impl Rule for AvoidGlobalState {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Variable(var) if is_mutable(var.keyword, var.is_late) => {
                    diags.push(ctx.diagnostic(
                        "avoid-global-state",
                        "Avoid mutable global state — use const or final declarations instead",
                        var.span,
                    ));
                }
                _ => {
                    if let Some(members) = crate::lint::declarations::members_of(decl) {
                        check_static_fields(members, &mut diags, ctx);
                    }
                }
            }
        }
        diags
    }
}

fn is_mutable(keyword: VarKeyword, is_late: bool) -> bool {
    keyword == VarKeyword::Var || (keyword == VarKeyword::Final && is_late)
}

fn check_static_fields(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in members {
        if let ClassMember::Field(field) = member
            && field.is_static
            && is_mutable(field.keyword, field.is_late)
        {
            diags.push(ctx.diagnostic(
                "avoid-global-state",
                "Avoid mutable global state — use const or final declarations instead",
                field.span,
            ));
        }
    }
}
