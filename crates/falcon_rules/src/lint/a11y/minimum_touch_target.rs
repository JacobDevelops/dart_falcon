//! Warn about explicitly undersized Flutter touch targets.
//!
//! `SizedBox` and Container are checked when their direct child is interactive.
//! `CupertinoButton` needs 44 logical pixels; other supported targets need 48.
//! Gestures without an active callback and explicitly disabled buttons are skipped.
//! Dynamic dimensions, indirect constraints, and non-interactive children are not inferred.
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MinimumTouchTarget;

impl Rule for MinimumTouchTarget {
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
        if !matches!(node, Expr::Call(_) | Expr::New(_)) {
            return;
        }
        let Some((identity, _, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !["SizedBox", "Container"]
            .iter()
            .any(|name| index.is_package_declaration(&identity, "flutter", name))
        {
            return;
        }
        let Some(child) = args.named().find(|arg| arg.name().name() == "child") else {
            return;
        };
        let Some((child_identity, _, child_args)) =
            crate::lint::flutter::scoped_constructor_invocation(child.value(), state)
        else {
            return;
        };
        if ![
            "IconButton",
            "FloatingActionButton",
            "GestureDetector",
            "InkWell",
            "InkResponse",
            "TextButton",
            "ElevatedButton",
            "OutlinedButton",
            "RawMaterialButton",
            "CupertinoButton",
        ]
        .iter()
        .any(|name| index.is_package_declaration(&child_identity, "flutter", name))
        {
            return;
        }
        if let Some(widget_name) = ["GestureDetector", "InkWell", "InkResponse"]
            .into_iter()
            .find(|name| index.is_package_declaration(&child_identity, "flutter", name))
            && !crate::lint::flutter::has_active_gesture_callback(child_args, widget_name)
        {
            return;
        }
        if [
            "IconButton",
            "FloatingActionButton",
            "TextButton",
            "ElevatedButton",
            "OutlinedButton",
            "RawMaterialButton",
            "CupertinoButton",
        ]
        .iter()
        .any(|name| index.is_package_declaration(&child_identity, "flutter", name))
            && child_args.named().any(|arg| {
                arg.name().name() == "onPressed"
                    && matches!(arg.value().unparenthesized(), Expr::NullLit(_))
            })
            && !child_args.named().any(|arg| {
                arg.name().name() == "onLongPress"
                    && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
            })
        {
            return;
        }
        let (minimum, integer_minimum, message) =
            if index.is_package_declaration(&child_identity, "flutter", "CupertinoButton") {
                (
                    44.0,
                    44,
                    "Provide a touch target at least 44 logical pixels wide and high.",
                )
            } else {
                (
                    48.0,
                    48,
                    "Provide a touch target at least 48 logical pixels wide and high.",
                )
            };
        let undersized = args.named().any(|arg| {
            if !matches!(arg.name().name(), "width" | "height") {
                return false;
            }
            literal_dimension(arg.value()).is_some_and(|value| match value {
                Dimension::Integer(value) => value < integer_minimum,
                Dimension::Double(value) => value < minimum,
            })
        });
        if undersized {
            self.diags.push(
                self.ctx
                    .diagnostic(message, crate::lint::flutter::constructor_span(node)),
            );
        }
    }
}

enum Dimension {
    Integer(i128),
    Double(f64),
}

fn literal_dimension(expression: Expr<'_>) -> Option<Dimension> {
    let (literal, negative) = match expression.unparenthesized() {
        Expr::Unary(unary) if unary.op() == UnaryOp::Minus => {
            (unary.operand().unparenthesized(), true)
        }
        literal => (literal, false),
    };
    match literal {
        Expr::IntLit(value) => falcon_analyze::parse_int(value.value())
            .and_then(|value| {
                if negative {
                    value.checked_neg()
                } else {
                    Some(value)
                }
            })
            .map(Dimension::Integer),
        Expr::DoubleLit(value) => value
            .value()
            .parse()
            .ok()
            .map(|value| Dimension::Double(if negative { -value } else { value })),
        _ => None,
    }
}
