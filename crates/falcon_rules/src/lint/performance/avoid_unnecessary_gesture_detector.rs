//! Flags handler-free Flutter GestureDetectors and tap-only wrappers around
//! enabled Flutter buttons. Explicit hit-test behavior is retained. The button
//! case requires an inline onPressed callback, so nullable handlers stay unflagged.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryGestureDetector;

impl Rule for AvoidUnnecessaryGestureDetector {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "GestureDetector") {
            return;
        }
        if args.named().any(|arg| {
            arg.name().name() == "behavior"
                && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
        }) {
            return;
        }
        let has_handler = args.named().any(|arg| {
            arg.name().name().starts_with("on")
                && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
        });
        let redundant_tap = has_handler
            && args.named().all(|arg| {
                matches!(arg.name().name(), "child" | "onTap")
                    || (arg.name().name().starts_with("on")
                        && matches!(arg.value().unparenthesized(), Expr::NullLit(_)))
            })
            && args
                .named()
                .find(|arg| arg.name().name() == "child")
                .and_then(|arg| {
                    crate::lint::flutter::scoped_constructor_invocation(arg.value(), state)
                })
                .is_some_and(|(child, constructor, child_args)| {
                    constructor == "new"
                        && [
                            "TextButton",
                            "ElevatedButton",
                            "OutlinedButton",
                            "IconButton",
                        ]
                        .iter()
                        .any(|name| index.is_package_declaration(&child, "flutter", name))
                        && child_args.named().any(|arg| {
                            arg.name().name() == "onPressed"
                                && matches!(arg.value().unparenthesized(), Expr::FuncExpr(_))
                        })
                });
        if !has_handler || redundant_tap {
            self.diagnostics.push(self.ctx.diagnostic(if redundant_tap {
                "Handle taps in the button's onPressed callback instead of wrapping it in GestureDetector."
            } else {
                "Remove this GestureDetector or provide an event handler."
            }, crate::lint::flutter::constructor_span(node)));
        }
    }
}
