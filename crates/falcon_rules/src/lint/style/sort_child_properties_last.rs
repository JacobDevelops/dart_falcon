//! Flags a `child` or `children` argument that is not the last named argument in a widget constructor call.
//!
//! In a Flutter widget tree the child is usually the largest and most deeply
//! nested argument, so keeping it last lets the eye follow the configuration
//! (padding, color, alignment) before descending into the subtree, and keeps the
//! closing `)` visually adjacent to the child's own closing bracket. This rule
//! flags any `child:`/`children:` named argument that appears before another
//! named argument. To avoid firing on ordinary function calls that happen to take
//! a `child` parameter, it only considers constructions whose callee is
//! PascalCase — implicit `Foo(...)` calls and explicit `new`/`const Foo(...)`.

use super::traversal::for_each_expr;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct SortChildPropertiesLast;

impl Rule for SortChildPropertiesLast {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_expr(program, |node| {
            if let Some((name, args, _)) = crate::lint::flutter::widget_construction(node)
                && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && !args.named.is_empty()
            {
                let last = args.named.len() - 1;
                for (i, named) in args.named.iter().enumerate() {
                    if matches!(named.name.name.as_str(), "child" | "children") && i != last {
                        diags.push(ctx.diagnostic(
                            "sort-child-properties-last",
                            "The child/children argument should be last in a widget constructor call.",
                            named.span,
                        ));
                    }
                }
            }
        });
        diags
    }
}
