//! Prefers `DecoratedBox` over a `Container` that only decorates its child.
//!
//! A `Container` whose only arguments are `decoration` and `child` (plus an
//! optional `key`) builds the same tree as a `DecoratedBox`, which is lighter
//! and has a `const` constructor. Only invocations that resolve to Flutter's
//! `Container` are considered, and any positional or other named argument
//! keeps the `Container`.
//!
//! ```dart
//! Container(decoration: BoxDecoration(), child: child);    // BAD
//! DecoratedBox(decoration: BoxDecoration(), child: child); // GOOD
//! ```
//!
//! Adopted from the official `use_decorated_box` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UseDecoratedBox;

impl Rule for UseDecoratedBox {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let mut collector = Collector {
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            container: DeclarationIdentity::flutter("Container"),
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    container: DeclarationIdentity,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((identity, "new", args)) =
            crate::lint::flutter::constructor_invocation(node, &self.model)
            && identity == self.container
            && only_decorates_child(args)
        {
            self.diags.push(self.ctx.diagnostic(
                "use-decorated-box",
                "Use 'DecoratedBox' rather than a 'Container' with only a 'Decoration'.",
                *node.span(),
            ));
        }
        walk_expr(self, node);
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
