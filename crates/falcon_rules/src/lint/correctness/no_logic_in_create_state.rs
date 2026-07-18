//! Require `createState` to do nothing but return a new `State` instance.
//!
//! Flags a `StatefulWidget`'s `createState` whose body is anything other than a
//! bare zero-argument construction of its `State` — that is, `=> _MyState()` or
//! `{ return _MyState(); }`. Flutter may call `createState` more than once over
//! a widget's lifetime and does not promise when, so initialization, argument
//! passing, or side effects placed here run at unpredictable moments and can
//! leak data between distinct `State` objects. Do that work in
//! `State.initState` or in the `State`'s field initializers instead, and pass
//! configuration through the widget rather than the constructor call.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoLogicInCreateState;

impl Rule for NoLogicInCreateState {
    fn name(&self) -> &'static str {
        "no-logic-in-create-state"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) if extends_stateful_widget(&c.extends) => {
                    check_members(&c.members, &mut diags, ctx);
                }
                TopLevelDecl::MixinClass(mc) if extends_stateful_widget(&mc.extends) => {
                    check_members(&mc.members, &mut diags, ctx);
                }
                _ => {}
            }
        }
        diags
    }
}

/// True when the superclass name is `StatefulWidget` (matched syntactically).
fn extends_stateful_widget(extends: &Option<DartType>) -> bool {
    matches!(extends, Some(DartType::Named(nt))
        if nt.segments.last().is_some_and(|s| s.name == "StatefulWidget"))
}

fn check_members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in members {
        if let ClassMember::Method(m) = member
            && m.name.name == "createState"
            && let Some(body) = &m.body
            && !is_trivial_state_construction(body)
        {
            diags.push(Diagnostic::new(
                "no-logic-in-create-state",
                Severity::Warning,
                "Avoid logic in createState; it should only return a new State instance.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: m.name.span.start,
                    end: m.name.span.end,
                },
            ));
        }
    }
}

/// True when the body is exactly `=> X()` or `{ return X(); }` where `X()` is a
/// zero-argument constructor call. A `Native`/absent body is treated as trivial
/// (nothing to inspect), so it is never flagged.
fn is_trivial_state_construction(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Arrow(expr, _) => is_zero_arg_construction(expr),
        FunctionBody::Block(block) => {
            matches!(block.stmts.as_slice(), [Stmt::Return(ReturnStmt { value: Some(expr), .. })]
                if is_zero_arg_construction(expr))
        }
        FunctionBody::Native(..) => true,
    }
}

/// True for a bare zero-argument constructor call such as `_MyState()`.
fn is_zero_arg_construction(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args, .. } => {
            matches!(callee.as_ref(), Expr::Ident(_))
                && args.positional.is_empty()
                && args.named.is_empty()
        }
        Expr::New { args, .. } => args.positional.is_empty() && args.named.is_empty(),
        _ => false,
    }
}
