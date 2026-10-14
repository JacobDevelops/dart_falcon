//! Flags a getter that unconditionally returns itself.
//!
//! A body of `=> x` (or `=> this.x`, or `return x;`) inside `get x` calls the
//! getter again with no base case, recursing until the stack overflows at
//! runtime. It almost always means a backing field was intended — typically an
//! underscore-prefixed `_x` — but was misspelled as the getter's own name.
//! Return the backing field instead. Only a direct, unconditional self-reference
//! is reported; a getter that merely mentions its own name inside a larger
//! expression is left alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct RecursiveGetters;

impl Rule for RecursiveGetters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(f)
                    if matches!(f.kind, falcon_syntax::ast::FunctionKind::Getter) =>
                {
                    check(&f.body, &f.name.name, &mut diags, ctx);
                }
                _ => {
                    if let Some(class_members) = crate::lint::declarations::members_of(decl) {
                        members(class_members, &mut diags, ctx);
                    }
                }
            }
        }
        diags
    }
}

fn members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for m in members {
        if let ClassMember::Getter(g) = m {
            check(&g.body, &g.name.name, diags, ctx);
        }
    }
}

fn check(
    body: &Option<FunctionBody>,
    name: &str,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let expr = match body.as_ref() {
        Some(FunctionBody::Arrow(e, _)) => Some(&**e),
        Some(FunctionBody::Block(b)) => match b.stmts.as_slice() {
            [Stmt::Return(r)] => r.value.as_ref(),
            _ => None,
        },
        _ => None,
    };
    let Some(expr) = expr else { return };
    if let Some(span) = self_reference(expr, name) {
        diags.push(ctx.diagnostic(
            "recursive-getters",
            "Recursive getter — this getter unconditionally returns itself",
            *span,
        ));
    }
}

/// The span of a direct self-reference (`x` or `this.x`), if `expr` is one.
fn self_reference<'a>(expr: &'a Expr, name: &str) -> Option<&'a Span> {
    match expr {
        Expr::Ident(id) if id.name == name => Some(&id.span),
        Expr::Field {
            object,
            field,
            span,
            ..
        } if matches!(&**object, Expr::This { .. }) && field.name == name => Some(span),
        _ => None,
    }
}
