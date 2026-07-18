//! Flags constructors that copy a plain parameter into a same-named field
//! (`prefer-initializing-formals`, adopted from package:lints). Both the
//! initializer-list form `: x = x` and the body form `this.x = x;` should be
//! replaced by an initializing formal `this.x`.
//!
//! Conservative: only exact `field = param` / `this.field = param` where the
//! parameter name equals the field name and the parameter is a plain (non-field,
//! non-super) constructor parameter.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use std::collections::HashSet;

pub struct PreferInitializingFormals;

impl Rule for PreferInitializingFormals {
    fn name(&self) -> &'static str {
        "prefer-initializing-formals"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                for member in members {
                    if let ClassMember::Constructor(ctor) = member {
                        check_ctor(ctor, ctx, &mut diags);
                    }
                }
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

fn check_ctor(ctor: &ConstructorDecl, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let params: HashSet<&str> = ctor
        .params
        .positional
        .iter()
        .chain(&ctor.params.optional_positional)
        .chain(&ctor.params.named)
        .filter(|p| !p.is_field && !p.is_super)
        .map(|p| p.name.name.as_str())
        .collect();

    // Initializer list: `: field = param`.
    for init in &ctor.initializers {
        if let ConstructorInitializer::FieldInit { field, value, .. } = init
            && let Expr::Ident(v) = value
            && v.name == field.name
            && params.contains(field.name.as_str())
        {
            push(diags, ctx, &field.span);
        }
    }

    // Body: `this.field = param;`.
    if let Some(FunctionBody::Block(block)) = &ctor.body {
        for stmt in &block.stmts {
            if let Stmt::Expr(expr_stmt) = stmt
                && let Expr::Assign {
                    target,
                    op: AssignOp::Eq,
                    value,
                    ..
                } = &expr_stmt.expr
                && let Expr::Field { object, field, .. } = target.as_ref()
                && matches!(object.as_ref(), Expr::This { .. })
                && let Expr::Ident(v) = value.as_ref()
                && v.name == field.name
                && params.contains(field.name.as_str())
            {
                push(diags, ctx, &field.span);
            }
        }
    }
}

fn push(diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, span: &Span) {
    diags.push(Diagnostic::new(
        "prefer-initializing-formals",
        Severity::Warning,
        "Use an initializing formal to assign a parameter to a field.".to_string(),
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
