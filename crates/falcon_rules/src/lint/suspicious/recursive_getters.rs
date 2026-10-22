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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct RecursiveGetters;

impl Rule for RecursiveGetters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Function(function) = declaration {
            if matches!(function.kind(), FunctionKind::Getter) {
                check(function.body(), function.name().name(), &mut self.diags, self.ctx);
            }
        } else if let Some(members) = crate::lint::declarations::members_of(declaration) {
            for member in members {
                if let ClassMember::Getter(getter) = member {
                    check(getter.body(), getter.name().name(), &mut self.diags, self.ctx);
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
    body: Option<FunctionBody<'_>>,
    name: &str,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let expr = match body {
        Some(FunctionBody::Arrow(arrow)) => Some(arrow.expr()),
        Some(FunctionBody::Block(b)) => {
            let mut stmts = b.stmts();
            match (stmts.next(), stmts.next()) {
                (Some(Stmt::Return(r)), None) => r.value(),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(expr) = expr else { return };
    if let Some(span) = self_reference(expr, name) {
        diags.push(ctx.diagnostic("Avoid this recursive getter because it unconditionally returns itself.", span));
    }
}

/// The span of a direct self-reference (`x` or `this.x`), if `expr` is one.
fn self_reference(expr: Expr<'_>, name: &str) -> Option<Span> {
    match expr.unparenthesized() {
        Expr::Ident(id) if id.name() == name => Some(id.span()),
        Expr::Field(field)
            if matches!(field.object().unparenthesized(), Expr::This(_))
                && field.field().name() == name =>
        {
            Some(field.span())
        }
        _ => None,
    }
}
