//! Warn about unlabeled interactive gesture targets.
//!
//! Checks resolved Flutter GestureDetector, InkWell, and InkResponse constructors.
//! Direct Text children, Icon semanticLabel, or Semantics/Tooltip ancestors with
//! plain or rich labels supply labels. Inert/excluded gestures and unknown/custom
//! children are skipped. Dynamic labels are accepted; null and empty literals are not.
//! This is a local heuristic, not compliance validation. Image labels and action
//! button tooltips are covered by avoid-missing-image-alt and
//! prefer-action-button-tooltip respectively.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AccessibleWidgetLabels;

impl Rule for AccessibleWidgetLabels {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            labeled: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    labeled: Vec<Span>,
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
        let Some(index) = self.ctx.identities else {
            return;
        };
        if ["Semantics", "Tooltip"]
            .iter()
            .any(|name| index.is_package_declaration(&identity, "flutter", name))
        {
            let labeled = args.named().any(|arg| {
                matches!(
                    arg.name().name(),
                    "label" | "attributedLabel" | "message" | "richMessage"
                ) && useful_label(arg.value())
            });
            if labeled && let Some(child) = args.named().find(|arg| arg.name().name() == "child") {
                self.labeled.push(child.value().span());
            }
            return;
        }
        if !["GestureDetector", "InkWell", "InkResponse"]
            .iter()
            .any(|name| index.is_package_declaration(&identity, "flutter", name))
        {
            return;
        }
        if !args.named().any(|arg| {
            matches!(
                arg.name().name(),
                "onTap"
                    | "onTapDown"
                    | "onTapUp"
                    | "onTapCancel"
                    | "onDoubleTap"
                    | "onDoubleTapDown"
                    | "onDoubleTapCancel"
                    | "onLongPress"
                    | "onLongPressStart"
                    | "onLongPressMoveUpdate"
                    | "onLongPressUp"
                    | "onLongPressEnd"
                    | "onLongPressDown"
                    | "onSecondaryTap"
                    | "onSecondaryTapDown"
                    | "onSecondaryTapUp"
                    | "onTertiaryTapDown"
                    | "onTertiaryTapUp"
                    | "onPanStart"
                    | "onPanUpdate"
                    | "onPanEnd"
                    | "onPanDown"
                    | "onScaleStart"
                    | "onScaleUpdate"
                    | "onScaleEnd"
                    | "onHorizontalDragStart"
                    | "onHorizontalDragUpdate"
                    | "onHorizontalDragEnd"
                    | "onVerticalDragStart"
                    | "onVerticalDragUpdate"
                    | "onVerticalDragEnd"
            ) && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
        }) {
            return;
        }
        if self
            .labeled
            .iter()
            .any(|span| span.start <= node.span().start && span.end >= node.span().end)
        {
            return;
        }
        if args.named().any(|arg| {
            arg.name().name() == "excludeFromSemantics"
                && matches!(arg.value().unparenthesized(), Expr::BoolLit(value) if value.value())
        }) {
            return;
        }
        let child = args.named().find(|arg| arg.name().name() == "child");
        if let Some(child) = child {
            let Some((child_identity, _, child_args)) =
                crate::lint::flutter::scoped_constructor_invocation(child.value(), state)
            else {
                return;
            };
            let text = index.is_package_declaration(&child_identity, "flutter", "Text");
            let icon = index.is_package_declaration(&child_identity, "flutter", "Icon");
            if !text && !icon {
                return;
            }
            if (text && child_args.positional().next().is_some_and(useful_label))
                || (icon
                    && child_args.named().any(|arg| {
                        arg.name().name() == "semanticLabel" && useful_label(arg.value())
                    }))
            {
                return;
            }
        }
        self.diags.push(self.ctx.diagnostic(
            "accessible-widget-labels",
            "Provide an accessible label for this interactive widget.",
            crate::lint::flutter::constructor_span(node),
        ));
    }
}

fn useful_label(expression: Expr<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::NullLit(_) => false,
        Expr::StringLit(string) => {
            string.interpolations().next().is_some()
                || string
                    .segments()
                    .any(|(delimiter, _)| !delimiter.content.trim().is_empty())
        }
        _ => true,
    }
}
