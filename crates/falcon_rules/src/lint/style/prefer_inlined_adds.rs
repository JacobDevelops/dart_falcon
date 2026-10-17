//! Flags an `add` cascade on a list literal whose element could be written inline.
//!
//! Building a list by literal-then-cascade (`[a]..add(b)`) is more verbose than
//! placing the element directly in the literal (`[a, b]`), and the inline form
//! reads as a single collection rather than a construction followed by mutation.
//! The rule matches each `..add(...)` section cascaded onto a list literal that
//! takes exactly one positional argument and no named arguments; multi-argument
//! or named calls, and cascades on non-list receivers, are ignored.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferInlinedAdds;

impl Rule for PreferInlinedAdds {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Cascade {
            object, sections, ..
        } = node.unparenthesized()
            && matches!(object.unparenthesized(), Expr::List { .. })
        {
            for section in sections {
                // Stop after one match because operations in a section share its span.
                for op in &section.ops {
                    if let CascadeOp::Call(ident, _, args) = op
                        && ident.name == "add"
                        && args.positional.len() == 1
                        && args.named.is_empty()
                    {
                        self.diags.push(self.ctx.diagnostic("prefer-inlined-adds", "Inline the added element into the collection literal instead of using 'add'.", section.span));
                        break;
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
