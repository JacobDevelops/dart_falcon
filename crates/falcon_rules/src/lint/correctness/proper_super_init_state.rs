//! Require `super.initState()` to be the first call in `initState`.
//!
//! Flags an `initState` method whose first statement is not `super.initState()`,
//! whether the call comes later or is missing altogether. The framework's
//! `State.initState` establishes the base bookkeeping the object depends on, so
//! a subclass must let it run before its own setup; initialization that touches
//! `context` or framework state before `super.initState()` runs against an
//! object that is not yet fully wired. Make `super.initState()` the first line
//! of the method.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ProperSuperInitState;

impl Rule for ProperSuperInitState {
    fn name(&self) -> &'static str {
        "proper-super-init-state"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) if extends_state(&c.extends) => {
                    for m in &c.members {
                        check_member(m, &mut diags, ctx);
                    }
                }
                TopLevelDecl::MixinClass(mc) if extends_state(&mc.extends) => {
                    for m in &mc.members {
                        check_member(m, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

/// True when a class extends a `State`-like base (its name ends with `State`,
/// e.g. `State`, `ConsumerState`). This scopes the lifecycle check to widgets.
fn extends_state(extends: &Option<DartType>) -> bool {
    matches!(extends, Some(DartType::Named(nt))
        if nt.segments.last().is_some_and(|s| s.name.ends_with("State")))
}

fn check_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let ClassMember::Method(m) = member
        && m.name.name == "initState"
        && let Some(FunctionBody::Block(block)) = &m.body
    {
        check_init_state(block, &m.name.span, diags, ctx);
    }
}

fn check_init_state(
    block: &Block,
    name_span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let first_is_super = matches!(
        block.stmts.first(),
        Some(Stmt::Expr(ExprStmt { expr, .. })) if is_super_init_state_call(expr)
    );
    if first_is_super {
        return;
    }

    // Either super.initState() appears but is not first, or it is missing.
    let report_span = block
        .stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Expr(ExprStmt { expr, span }) if is_super_init_state_call(expr) => Some(span),
            _ => None,
        })
        .unwrap_or(name_span);

    diags.push(Diagnostic::new(
        "proper-super-init-state",
        Severity::Warning,
        "super.initState() should be called first in initState().",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: report_span.start,
            end: report_span.end,
        },
    ));
}

fn is_super_init_state_call(expr: &Expr) -> bool {
    if let Expr::Call { callee, .. } = expr
        && let Expr::Field { object, field, .. } = callee.as_ref()
        && matches!(object.as_ref(), Expr::Super { .. })
        && field.name == "initState"
    {
        return true;
    }
    false
}
