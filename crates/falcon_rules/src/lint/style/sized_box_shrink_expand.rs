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

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation, parse_int};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::expressions::is_parenthesized;

#[derive(Default)]
pub struct SizedBoxShrinkExpand;

impl Rule for SizedBoxShrinkExpand {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            sized_box: DeclarationIdentity::flutter("SizedBox"),
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    sized_box: DeclarationIdentity,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
            && identity == self.sized_box
            && !matches!(node, Expr::Call { callee, .. }
                if matches!(callee.as_ref(), Expr::Field { field, .. } if field.name == "new"))
            && !matches!(node, Expr::New { dart_type: DartType::Named(named), constructor_name, .. }
                if constructor_name.is_some()
                    || named.segments.last().is_some_and(|name| name.name == "new"))
            && let Some(replacement) = replacement(args)
        {
            self.diags.push(self.ctx.diagnostic(
                "sized-box-shrink-expand",
                format!(
                    "Use 'SizedBox.{replacement}' to avoid needing to specify the 'height' and 'width'."
                ),
                crate::lint::flutter::constructor_span(node),
            ));
        }
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

/// The value of a bare numeric literal or `double.infinity`; parenthesized
/// values do not match the official rule.
fn literal_value(expression: &Expr) -> Option<f64> {
    if is_parenthesized(expression) {
        return None;
    }
    match expression {
        Expr::IntLit { value, .. } => parse_int(value).map(|value| value as f64),
        Expr::DoubleLit { value, .. } => value.replace('_', "").parse().ok(),
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } if field.name == "infinity"
            && matches!(object.as_ref(), Expr::Ident(prefix) if prefix.name == "double")
            && !is_parenthesized(object) =>
        {
            Some(f64::INFINITY)
        }
        _ => None,
    }
}
