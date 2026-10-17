//! Flags an `addAll` cascade on a collection literal that a spread would express.
//!
//! The spread operator (`[a, b, ...x]`) folds another collection's elements into
//! a literal in one expression, so `[a, b]..addAll(x)` is unnecessarily verbose
//! and reads as a construction followed by mutation rather than a single
//! collection. The rule matches each `..addAll(...)` section cascaded onto a
//! list, set, or map literal that takes exactly one positional argument and no
//! named arguments; other shapes and non-literal receivers are ignored.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferSpreadCollections;

impl Rule for PreferSpreadCollections {
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
    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Cascade {
            object, sections, ..
        } = node.unparenthesized()
            && is_collection_literal(object)
        {
            for section in sections {
                // Stop after one match because operations in a section share its span.
                for op in &section.ops {
                    if let CascadeOp::Call(ident, _, args) = op
                        && ident.name == "addAll"
                        && args.positional.len() == 1
                        && args.named.is_empty()
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "prefer-spread-collections",
                            "Use the spread operator '...' instead of 'addAll'.",
                            section.span,
                        ));
                        break;
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_collection_literal(expr: &Expr) -> bool {
    matches!(
        expr.unparenthesized(),
        Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. }
    )
}
