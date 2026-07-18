//! Flags a trivial getter/setter pair that only exposes a private field
//! (`unnecessary-getters-setters`, adopted from package:lints). When the getter
//! just returns `_x` and the setter just assigns `_x`, the field should be
//! public instead.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct UnnecessaryGettersSetters;

impl Rule for UnnecessaryGettersSetters {
    fn name(&self) -> &'static str {
        "unnecessary-getters-setters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                check_members(members, ctx, &mut diags);
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::Mixin(m) => Some(&m.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

fn check_members(members: &[ClassMember], ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let mut getters: HashMap<&str, &GetterDecl> = HashMap::new();
    let mut setters: HashMap<&str, &SetterDecl> = HashMap::new();
    for member in members {
        match member {
            ClassMember::Getter(g) => {
                getters.insert(g.name.name.as_str(), g);
            }
            ClassMember::Setter(s) => {
                setters.insert(s.name.name.as_str(), s);
            }
            _ => {}
        }
    }

    for (name, getter) in &getters {
        let Some(setter) = setters.get(name) else {
            continue;
        };
        let (Some(gf), Some(sf)) = (getter_returns_field(getter), setter_assigns_field(setter))
        else {
            continue;
        };
        // Both accessors must wrap the same private backing field.
        if gf == sf && gf.starts_with('_') {
            diags.push(Diagnostic::new(
                "unnecessary-getters-setters",
                Severity::Warning,
                "Avoid wrapping a field in trivial getters and setters; use a public field."
                    .to_string(),
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: getter.name.span.start,
                    end: getter.name.span.end,
                },
            ));
        }
    }
}

fn field_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(id.name.as_str()),
        Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::This { .. }) => {
            Some(field.name.as_str())
        }
        _ => None,
    }
}

fn getter_returns_field(getter: &GetterDecl) -> Option<&str> {
    match getter.body.as_ref()? {
        FunctionBody::Arrow(expr, _) => field_name(expr),
        FunctionBody::Block(block) if block.stmts.len() == 1 => match &block.stmts[0] {
            Stmt::Return(ret) => field_name(ret.value.as_ref()?),
            _ => None,
        },
        _ => None,
    }
}

fn setter_assigns_field(setter: &SetterDecl) -> Option<&str> {
    let assign = match setter.body.as_ref()? {
        FunctionBody::Arrow(expr, _) => expr.as_ref(),
        FunctionBody::Block(block) if block.stmts.len() == 1 => match &block.stmts[0] {
            Stmt::Expr(e) => &e.expr,
            _ => return None,
        },
        _ => return None,
    };
    if let Expr::Assign {
        target,
        op: AssignOp::Eq,
        value,
        ..
    } = assign
        && let Expr::Ident(rhs) = value.as_ref()
        && rhs.name == setter.param.name
    {
        return field_name(target);
    }
    None
}
