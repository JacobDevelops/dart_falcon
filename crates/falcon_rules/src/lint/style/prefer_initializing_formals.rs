//! Flags a constructor that copies a plain parameter straight into a field
//! (`prefer-initializing-formals`, adopted from package:lints).
//!
//! Dart's initializing formals (`this.x`) declare the parameter and assign the
//! field in one step, so the manual copy — whether an initializer-list entry
//! `: x = x` or a body statement `this.x = x;` — is redundant boilerplate. The
//! rule is intentionally narrow: it matches only an exact `field = param` /
//! `this.field = param` copy of a plain constructor parameter, never an existing
//! initializing (`this.`) or super (`super.`) formal, so cases that already use
//! the idiom or that transform the value are left untouched.
//!
//! A *named* parameter must already carry the field's name — renaming it would
//! break callers — while a *positional* one may be named anything, since
//! promoting it to `this.field` does not change the call-site API. A parameter
//! copied into more than one field is skipped: it cannot become an initializing
//! formal.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct PreferInitializingFormals;

impl Rule for PreferInitializingFormals {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = crate::lint::declarations::members_of(decl) {
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

fn check_ctor(ctor: &ConstructorDecl, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let plain = |p: &&FormalParam| p.initializing.is_none();
    let positional: HashSet<&str> = ctor
        .params
        .positional()
        .filter(plain)
        .map(|p| p.name.name.as_str())
        .collect();
    // A named parameter's name is part of the call-site API, so converting it to
    // an initializing formal is only non-breaking when it already matches the field.
    let named: HashSet<&str> = ctor
        .params
        .named()
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
            diags.push(ctx.diagnostic(
                "prefer-initializing-formals",
                "Use an initializing formal to assign a parameter to a field.".to_string(),
                field.span,
            ));
        }
    }
}
