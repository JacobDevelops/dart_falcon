//! Flags an `@override` member that does nothing but forward to `super` with
//! the same arguments.
//!
//! Such an override adds visual noise and a maintenance burden without changing
//! behavior — delete it and the inherited implementation stays in force. The
//! rule is deliberately conservative: it only considers members annotated
//! `@override` (the annotation stands in for real override resolution), and it
//! leaves an override alone when it carries an extra annotation or declares
//! parameter defaults or `covariant`, since those signal an intentional
//! contract change. Forwarding methods (`super.m(args)`), getters (`super.x`),
//! and setters (`super.x = v`) are all checked; operator overrides are not.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UnnecessaryOverrides;

impl Rule for UnnecessaryOverrides {
    fn name(&self) -> &'static str {
        "unnecessary-overrides"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let members = match decl {
                TopLevelDecl::Class(c) => &c.members,
                TopLevelDecl::Mixin(m) => &m.members,
                TopLevelDecl::MixinClass(mc) => &mc.members,
                TopLevelDecl::Enum(e) => &e.members,
                TopLevelDecl::Extension(e) => &e.members,
                TopLevelDecl::ExtensionType(e) => &e.members,
                _ => continue,
            };
            for member in members {
                check_member(member, &mut diags, ctx);
            }
        }
        diags
    }
}

fn check_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let (annotations, name_span, unnecessary) = match member {
        ClassMember::Method(m) => (&m.annotations, &m.name.span, method_forwards(m)),
        ClassMember::Getter(g) => (&g.annotations, &g.name.span, getter_forwards(g)),
        ClassMember::Setter(s) => (&s.annotations, &s.name.span, setter_forwards(s)),
        _ => return,
    };
    if !only_override(annotations) || !unnecessary {
        return;
    }
    diags.push(Diagnostic::new(
        "unnecessary-overrides",
        Severity::Warning,
        "Unnecessary override — this member only forwards to `super` unchanged",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: name_span.start,
            end: name_span.end,
        },
    ));
}

/// True when the member is annotated `@override` and carries no other
/// annotation (an extra annotation means the override adds meaning).
fn only_override(annotations: &[Annotation]) -> bool {
    let mut saw_override = false;
    for a in annotations {
        let is_override = a.name.last().is_some_and(|id| id.name == "override") && a.args.is_none();
        if is_override {
            saw_override = true;
        } else {
            return false;
        }
    }
    saw_override
}

fn params_add_meaning(params: &FormalParamList) -> bool {
    params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
        .any(|p| p.default_value.is_some() || p.is_covariant)
}

fn args_match_params(args: &ArgList, params: &FormalParamList) -> bool {
    let positional: Vec<&str> = params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .map(|p| p.name.name.as_str())
        .collect();
    if args.positional.len() != positional.len() {
        return false;
    }
    for (arg, expected) in args.positional.iter().zip(positional) {
        match arg {
            Expr::Ident(id) if id.name == expected => {}
            _ => return false,
        }
    }
    if args.named.len() != params.named.len() {
        return false;
    }
    for np in &params.named {
        let Some(na) = args.named.iter().find(|na| na.name.name == np.name.name) else {
            return false;
        };
        match &na.value {
            Expr::Ident(id) if id.name == np.name.name => {}
            _ => return false,
        }
    }
    true
}

fn is_super(expr: &Expr) -> bool {
    matches!(expr, Expr::Super { .. })
}

/// The single expression a body forwards, if the body is exactly one
/// forwarding statement (arrow, `return e;`, or a bare `e;`).
fn single_expr(body: &Option<FunctionBody>) -> Option<&Expr> {
    match body.as_ref()? {
        FunctionBody::Arrow(e, _) => Some(e),
        FunctionBody::Block(b) => match b.stmts.as_slice() {
            [Stmt::Return(r)] => r.value.as_ref(),
            [Stmt::Expr(e)] => Some(&e.expr),
            _ => None,
        },
        FunctionBody::Native(_, _) => None,
    }
}

fn method_forwards(m: &MethodDecl) -> bool {
    if params_add_meaning(&m.params) {
        return false;
    }
    let Some(expr) = single_expr(&m.body) else {
        return false;
    };
    match expr {
        Expr::Call { callee, args, .. } => match &**callee {
            Expr::Field { object, field, .. } => {
                is_super(object) && field.name == m.name.name && args_match_params(args, &m.params)
            }
            _ => false,
        },
        _ => false,
    }
}

fn getter_forwards(g: &GetterDecl) -> bool {
    let Some(expr) = single_expr(&g.body) else {
        return false;
    };
    match expr {
        Expr::Field { object, field, .. } => is_super(object) && field.name == g.name.name,
        _ => false,
    }
}

fn setter_forwards(s: &SetterDecl) -> bool {
    let Some(expr) = single_expr(&s.body) else {
        return false;
    };
    match expr {
        Expr::Assign {
            target,
            op: AssignOp::Eq,
            value,
            ..
        } => {
            let target_ok = matches!(
                &**target,
                Expr::Field { object, field, .. } if is_super(object) && field.name == s.name.name
            );
            let value_ok = matches!(&**value, Expr::Ident(id) if id.name == s.param.name);
            target_ok && value_ok
        }
        _ => false,
    }
}
