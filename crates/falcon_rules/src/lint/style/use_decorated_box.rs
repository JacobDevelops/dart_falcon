//! Prefers `DecoratedBox` over a `Container` that only decorates its child.
//!
//! A `Container` whose only arguments are `decoration` and `child` (plus an
//! optional `key`) builds the same tree as a `DecoratedBox`, which is lighter
//! and has a `const` constructor. Only invocations that resolve to Flutter's
//! `Container` are considered, including declarations indexed while analyzing
//! Flutter itself. Any positional or other named argument keeps the `Container`.
//!
//! ```dart
//! Container(decoration: BoxDecoration(), child: child);    // BAD
//! DecoratedBox(decoration: BoxDecoration(), child: child); // GOOD
//! ```
//!
//! Adopted from the official `use_decorated_box` lint.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseDecoratedBox;

impl Rule for UseDecoratedBox {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
            && self.ctx.identities().is_some_and(|index| {
                index.is_package_declaration(&identity, "flutter", "Container")
            })
            && only_decorates_child(args)
        {
            self.diags.push(self.ctx.diagnostic("Use `DecoratedBox` rather than a `Container` with only a `Decoration`.", crate::lint::flutter::constructor_span(node)));
        }
    }
}

fn only_decorates_child(args: ArgList<'_>) -> bool {
    let names = || args.named().map(|argument| argument.name().name());
    args.positional().is_empty()
        && names().all(|name| matches!(name, "child" | "decoration" | "key"))
        && names().any(|name| name == "child")
        && names().any(|name| name == "decoration")
}
