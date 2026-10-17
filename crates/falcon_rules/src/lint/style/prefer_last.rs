//! Flags manual last-element indexing in favor of the `.last` getter.
//!
//! Catches the `xs[xs.length - 1]` idiom, where the receiver and the collection
//! whose `length` is read are the same identifier. Iterables expose a dedicated
//! `.last` getter that reads more clearly, avoids repeating the receiver, and
//! removes the off-by-one arithmetic that manual indexing invites. The match is
//! deliberately narrow: it requires a literal `- 1` subtracted from `<name>.length`
//! indexing `<name>`, so unrelated index expressions are left alone.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferLast;

impl Rule for PreferLast {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Index(index) = node
            && is_length_minus_one(index.object(), index.index())
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-last",
                "Prefer .last over [length - 1] to access the last element",
                index.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}

/// Matches `ident[ident.length - 1]` where both idents are the same name.
fn is_length_minus_one(object: Expr<'_>, index: Expr<'_>) -> bool {
    let obj_name = match object.unparenthesized() {
        Expr::Ident(id) => id.name(),
        _ => return false,
    };
    match index.unparenthesized() {
        Expr::Binary(binary) if binary.op() == BinaryOp::Sub => {
            if !matches!(binary.right().unparenthesized(), Expr::IntLit(value) if value.value() == "1")
            {
                return false;
            }
            match binary.left().unparenthesized() {
                Expr::Field(field) => {
                    field.field().name() == "length"
                        && matches!(field.object().unparenthesized(), Expr::Ident(id) if id.name() == obj_name)
                }
                _ => false,
            }
        }
        _ => false,
    }
}
