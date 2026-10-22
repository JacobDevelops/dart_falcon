//! Flags `Map.fromIterable(x, key: ..., value: ...)`.
//!
//! A collection-`for` element inside a map literal —
//! `{ for (final e in x) key(e): value(e) }` — expresses the same
//! transformation more directly, can be const, and avoids the intermediate
//! closures `Map.fromIterable` requires. The rule fires on both
//! `Map.fromIterable(...)` and `new Map.fromIterable(...)` when at least one
//! positional argument and both the `key:` and `value:` named arguments are
//! present.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferForElementsToMapFromIterable;

impl Rule for PreferForElementsToMapFromIterable {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            // `Map.fromIterable(...)` — parsed as a call on a `Map.fromIterable`
            // member reference.
            Expr::Call(call) => {
                let args = call.args();
                if let Expr::Field(field) = call.callee().unparenthesized()
                    && let Expr::Ident(id) = field.object().unparenthesized()
                    && id.name() == "Map"
                    && field.field().name() == "fromIterable"
                    && !args.positional().is_empty()
                    && has_named(args, "key")
                    && has_named(args, "value")
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Use a map literal with a `for` element instead of `Map.fromIterable`.",
                        field.field().span(),
                    ));
                }
            }
            // `new Map.fromIterable(...)`.
            Expr::New(construction) if matches!(construction.dart_type(), DartType::Named(nt)
                    if nt.segments().last().map(|s| s.name()) == Some("Map"))
                && construction.constructor_name().is_some_and(|ctor| ctor.name() == "fromIterable")
                && !construction.args().positional().is_empty()
                && has_named(construction.args(), "key")
                && has_named(construction.args(), "value") =>
            {
                self.diags.push(self.ctx.diagnostic(
                    "Use a map literal with a `for` element instead of `Map.fromIterable`.",
                    construction.span(),
                ));
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

fn has_named(args: ArgList<'_>, name: &str) -> bool {
    args.named().any(|n| n.name().name() == name)
}
