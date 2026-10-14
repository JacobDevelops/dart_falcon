//! Prefers `SizedBox.shrink()` / `SizedBox.expand()` over a `SizedBox` sized to zero or infinity.
//!
//! The unnamed `SizedBox` constructor with `width: 0, height: 0` is exactly
//! `SizedBox.shrink`, and with `width: double.infinity, height: double.infinity`
//! it is exactly `SizedBox.expand`; the named constructors state the intent
//! directly. Only invocations that resolve to Flutter's `SizedBox` and pass no
//! positional arguments are considered; other named arguments (`child`, `key`)
//! carry over unchanged.
//!
//! ```dart
//! SizedBox(width: 0, height: 0, child: logo);  // BAD
//! SizedBox.shrink(child: logo);                // GOOD
//! ```
//!
//! Adopted from the official `sized_box_shrink_expand` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel, parse_int};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct SizedBoxShrinkExpand;

impl Rule for SizedBoxShrinkExpand {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let mut collector = Collector {
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            sized_box: DeclarationIdentity::flutter("SizedBox"),
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    sized_box: DeclarationIdentity,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((identity, "new", args)) =
            crate::lint::flutter::constructor_invocation(node, &self.model)
            && identity == self.sized_box
            && let Some(replacement) = replacement(args)
        {
            self.diags.push(self.ctx.diagnostic(
                "sized-box-shrink-expand",
                format!(
                    "Use 'SizedBox.{replacement}' to avoid needing to specify the 'height' and 'width'."
                ),
                *node.span(),
            ));
        }
        walk_expr(self, node);
    }
}

/// The named constructor equivalent to these arguments, if any.
fn replacement(args: &ArgList) -> Option<&'static str> {
    if !args.positional.is_empty() {
        return None;
    }
    let dimension = |name: &str| {
        args.named
            .iter()
            .find(|argument| argument.name.name == name)
            .and_then(|argument| literal_value(&argument.value))
    };
    match (dimension("width")?, dimension("height")?) {
        (width, height) if width == 0.0 && height == 0.0 => Some("shrink"),
        (width, height) if width == f64::INFINITY && height == f64::INFINITY => Some("expand"),
        _ => None,
    }
}

/// The value of an integer or double literal, or of `double.infinity`.
fn literal_value(expression: &Expr) -> Option<f64> {
    match expression {
        Expr::IntLit { value, .. } => parse_int(value).map(|value| value as f64),
        Expr::DoubleLit { value, .. } => value.replace('_', "").parse().ok(),
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } if field.name == "infinity"
            && matches!(object.as_ref(), Expr::Ident(prefix) if prefix.name == "double") =>
        {
            Some(f64::INFINITY)
        }
        _ => None,
    }
}
