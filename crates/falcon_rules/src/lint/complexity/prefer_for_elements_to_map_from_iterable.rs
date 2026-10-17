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
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferForElementsToMapFromIterable;

impl Rule for PreferForElementsToMapFromIterable {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
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
        match node {
            // `Map.fromIterable(...)` — parsed as a call on a `Map.fromIterable`
            // member reference.
            Expr::Call { callee, args, .. } => {
                if let Expr::Field { object, field, .. } = callee.unparenthesized()
                    && let Expr::Ident(id) = object.unparenthesized()
                    && id.name == "Map"
                    && field.name == "fromIterable"
                    && !args.positional.is_empty()
                    && has_named(args, "key")
                    && has_named(args, "value")
                {
                    self.diags.push(self.ctx.diagnostic(
                        "prefer-for-elements-to-map-from-iterable",
                        "Use a map literal with a 'for' element instead of 'Map.fromIterable'.",
                        field.span,
                    ));
                }
            }
            // `new Map.fromIterable(...)`.
            Expr::New {
                dart_type: DartType::Named(nt),
                constructor_name: Some(ctor),
                args,
                span,
                ..
            } if nt.segments.last().map(|s| s.name.as_str()) == Some("Map")
                && ctor.name == "fromIterable"
                && !args.positional.is_empty()
                && has_named(args, "key")
                && has_named(args, "value") =>
            {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-for-elements-to-map-from-iterable",
                    "Use a map literal with a 'for' element instead of 'Map.fromIterable'.",
                    *span,
                ));
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

fn has_named(args: &ArgList, name: &str) -> bool {
    args.named.iter().any(|n| n.name.name == name)
}
