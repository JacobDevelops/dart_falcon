//! Flags a getter that unconditionally returns itself.
//!
//! A body of `=> x` (or `=> this.x`, or `return x;`) inside `get x` calls the
//! getter again with no base case, recursing until the stack overflows at
//! runtime. It almost always means a backing field was intended — typically an
//! underscore-prefixed `_x` — but was misspelled as the getter's own name.
//! Return the backing field instead. Only a direct, unconditional self-reference
//! is reported; a getter that merely mentions its own name inside a larger
//! expression is left alone.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct RecursiveGetters;

impl Rule for RecursiveGetters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, declaration: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Function(function) = declaration {
            if matches!(function.kind, FunctionKind::Getter) {
                check(&function.body, &function.name.name, &mut self.diags, self.ctx);
            }
        } else if let Some(members) = crate::lint::declarations::members_of(declaration) {
            for member in members {
                if let ClassMember::Getter(getter) = member {
                    check(&getter.body, &getter.name.name, &mut self.diags, self.ctx);
                }
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
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
    match expr.unparenthesized() {
        Expr::Ident(id) if id.name == name => Some(&id.span),
        Expr::Field {
            object,
            field,
            span,
            ..
        } if matches!(object.unparenthesized(), Expr::This { .. }) && field.name == name => {
            Some(span)
        }
        _ => None,
    }
}
