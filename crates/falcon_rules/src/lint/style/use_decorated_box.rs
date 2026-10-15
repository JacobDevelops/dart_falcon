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

use crate::lint::semantic_scope::{self, SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseDecoratedBox;

impl Rule for UseDecoratedBox {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        semantic_scope::visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
            && self.ctx.identities.is_some_and(|index| {
                index.is_package_declaration(&identity, "flutter", "Container")
            })
            && only_decorates_child(args)
        {
            self.diags.push(self.ctx.diagnostic(
                "use-decorated-box",
                "Use 'DecoratedBox' rather than a 'Container' with only a 'Decoration'.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
    }
}

fn only_decorates_child(args: &ArgList) -> bool {
    let names = || {
        args.named
            .iter()
            .map(|argument| argument.name.name.as_str())
    };
    args.positional.is_empty()
        && names().all(|name| matches!(name, "child" | "decoration" | "key"))
        && names().any(|name| name == "child")
        && names().any(|name| name == "decoration")
}
