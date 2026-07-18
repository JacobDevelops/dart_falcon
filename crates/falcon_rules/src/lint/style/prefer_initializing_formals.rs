//! Flags constructors that copy a plain parameter straight into a field
//! (`prefer-initializing-formals`, adopted from package:lints). Both the
//! initializer-list form `: x = x` and the body form `this.x = x;` should be
//! replaced by an initializing formal `this.x`.
//!
//! Conservative: only exact `field = param` / `this.field = param` where the
//! parameter is a plain (non-field, non-super) constructor parameter. A named
//! parameter must already carry the field's name — renaming it would break
//! callers — while a positional one may be named anything. A parameter copied
//! into more than one field is skipped: it cannot become an initializing formal.

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
    let plain = |p: &&FormalParam| !p.is_field && !p.is_super;
    let positional: HashSet<&str> = ctor
        .params
        .positional
        .iter()
        .chain(&ctor.params.optional_positional)
        .filter(plain)
        .map(|p| p.name.name.as_str())
        .collect();
    // A named parameter's name is part of the call-site API, so converting it to
    // an initializing formal is only non-breaking when it already matches the field.
    let named: HashSet<&str> = ctor
        .params
        .named
        .iter()
        .filter(plain)
        .map(|p| p.name.name.as_str())
        .collect();

    // (field, parameter name) for every `field = param` / `this.field = param`.
    let mut copies: Vec<(&Identifier, &str)> = Vec::new();

    for init in &ctor.initializers {
        if let ConstructorInitializer::FieldInit { field, value, .. } = init
            && let Expr::Ident(v) = value
        {
            copies.push((field, v.name.as_str()));
        }
    }
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
            {
                copies.push((field, v.name.as_str()));
            }
        }
    }

    for (i, (field, param)) in copies.iter().enumerate() {
        // One parameter feeding two fields cannot become an initializing formal.
        if copies
            .iter()
            .enumerate()
            .any(|(j, (_, p))| j != i && p == param)
        {
            continue;
        }
        if positional.contains(param) || (named.contains(param) && *param == field.name) {
            push(diags, ctx, &field.span);
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
