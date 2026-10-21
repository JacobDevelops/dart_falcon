//! Warn about explicitly undersized Flutter touch targets.
//!
//! SizedBox and Container are checked when their direct child is interactive.
//! CupertinoButton needs 44 logical pixels; other supported targets need 48.
//! Gestures without an active callback and explicitly disabled buttons are skipped.
//! Dynamic dimensions, indirect constraints, and non-interactive children are not inferred.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
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
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
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
        let minimum = if index.is_package_declaration(&child_identity, "flutter", "CupertinoButton")
        {
            44.0
        } else {
            48.0
        };
        let undersized = args.named().any(|arg| {
            if !matches!(arg.name().name(), "width" | "height") {
                return false;
            }
            literal_dimension(arg.value()).is_some_and(|value| value < minimum)
        });
        if undersized {
            self.diags.push(self.ctx.diagnostic(if minimum == 44.0 {
                "Provide a touch target at least 44 logical pixels wide and high."
            } else {
                "Provide a touch target at least 48 logical pixels wide and high."
            }, crate::lint::flutter::constructor_span(node)));
        }
    }
}

fn literal_dimension(expression: Expr<'_>) -> Option<f64> {
    let (literal, sign) = match expression.unparenthesized() {
        Expr::Unary(unary) if unary.op() == UnaryOp::Minus => {
            (unary.operand().unparenthesized(), -1.0)
        }
        literal => (literal, 1.0),
    };
    let value = match literal {
        Expr::IntLit(value) => falcon_analyze::parse_int(value.value()).map(|value| value as f64),
        Expr::DoubleLit(value) => value.value().parse().ok(),
        _ => None,
    };
    value.map(|value| sign * value)
}
