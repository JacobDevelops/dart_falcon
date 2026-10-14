//! Prefers `ColoredBox` over a `Container` that only paints a color behind its child.
//!
//! A `Container` whose only arguments are a non-nullable `color` and a `child`
//! (plus an optional `key`) builds the same tree as a `ColoredBox`, which is
//! lighter and has a `const` constructor. Only invocations that resolve to
//! Flutter's `Container` are considered. A `color` that may be `null` keeps the
//! `Container` (a `ColoredBox` requires a color), so the rule only fires when
//! the color is provably non-nullable: a non-nullable static type, a
//! constructor invocation, or a `Colors.<name>` constant.
//!
//! ```dart
//! Container(color: Colors.blue, child: child);  // BAD
//! ColoredBox(color: Colors.blue, child: child); // GOOD
//! ```
//!
//! Adopted from the official `use_colored_box` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseColoredBox;

impl Rule for UseColoredBox {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
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
            crate::lint::flutter::constructor_invocation(node, &state.model)
            && identity == DeclarationIdentity::flutter("Container")
            && only_colors_child(args, state)
        {
            self.diags.push(self.ctx.diagnostic(
                "use-colored-box",
                "Use a 'ColoredBox' rather than a 'Container' with only a 'Color'.",
                *node.span(),
            ));
        }
    }
}

fn only_colors_child(args: &ArgList, state: &SemanticState<'_>) -> bool {
    let mut has_child = false;
    let mut has_color = false;
    for argument in &args.named {
        match argument.name.name.as_str() {
            "child" => has_child = true,
            "color" if non_nullable_color(&argument.value, state) => has_color = true,
            "key" => {}
            _ => return false,
        }
    }
    args.positional.is_empty() && has_child && has_color
}

fn non_nullable_color(color: &Expr, state: &SemanticState<'_>) -> bool {
    match state.infer(color) {
        ResolvedType::Unknown => {}
        ty => return !ty.nullable() || ty == ResolvedType::Dynamic,
    }
    if crate::lint::flutter::constructor_invocation(color, &state.model).is_some() {
        return true;
    }
    // Every static member of Flutter's `Colors` is a non-nullable color.
    matches!(
        color,
        Expr::Field { object, is_null_safe: false, .. }
            if matches!(object.as_ref(), Expr::Ident(prefix)
                if !state.environment.is_bound(&prefix.name)
                    && state.model.resolve_name(&prefix.name)
                        == Some(DeclarationIdentity::flutter("Colors")))
    )
}
