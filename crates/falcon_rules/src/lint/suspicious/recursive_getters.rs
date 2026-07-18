//! Flags getters that unconditionally return themselves, ported from
//! package:lints `recursive_getters`. `int get x => x;` (or `=> this.x`) recurses
//! forever. Only the direct, unconditional self-reference is reported, so a
//! getter that merely mentions its own name inside a larger expression is left
//! alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct RecursiveGetters;

impl Rule for RecursiveGetters {
    fn name(&self) -> &'static str {
        "recursive-getters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(f) if f.is_getter => {
                    check(&f.body, &f.name.name, &mut diags, ctx);
                }
                TopLevelDecl::Class(c) => members(&c.members, &mut diags, ctx),
                TopLevelDecl::Mixin(m) => members(&m.members, &mut diags, ctx),
                TopLevelDecl::MixinClass(mc) => members(&mc.members, &mut diags, ctx),
                TopLevelDecl::Enum(e) => members(&e.members, &mut diags, ctx),
                TopLevelDecl::Extension(e) => members(&e.members, &mut diags, ctx),
                TopLevelDecl::ExtensionType(e) => members(&e.members, &mut diags, ctx),
                _ => {}
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
        diags.push(Diagnostic::new(
            "recursive-getters",
            Severity::Warning,
            "Recursive getter — this getter unconditionally returns itself",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
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
