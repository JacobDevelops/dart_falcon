//! Avoid singleton collection literals where SDK APIs accept a single value.
//!
//! Only plain singleton lists/sets and semantically resolved SDK receivers are
//! checked. Collection control flow, spreads, maps, dynamic/custom APIs and named
//! arguments are excluded. Future.wait is checked only for a discarded awaited
//! result, because replacing it changes List<T> to T. Future.any is excluded when
//! its result is discarded, to preserve its error listener.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, expression_segments};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::{scoped_constructor_invocation, value_name_is_shadowed};

#[derive(Default)]
pub struct AvoidUnnecessaryCollections;

impl Rule for AvoidUnnecessaryCollections {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let (args, replacement) = if let Some((identity, name, args)) =
            scoped_constructor_invocation(node, state)
            && identity.is_sdk("dart:async", "Stream")
        {
            let replacement = match name {
                "fromFutures" => "Stream.fromFuture",
                "fromIterable" => "Stream.value",
                _ => return,
            };
            (args, Some(replacement))
        } else {
            let Expr::Call(call) = node else {
                return;
            };
            let Expr::Field(field) = call.callee().unparenthesized() else {
                return;
            };
            if field.is_null_safe() {
                return;
            }
            let receiver = state.infer(field.object());
            let method = field.field().name();
            let replacement = if (receiver.interface("dart:core", "List")
                || receiver.interface("dart:core", "Set"))
                && method == "addAll"
            {
                Some("add")
            } else if receiver.interface("dart:core", "Set") && method == "containsAll" {
                Some("contains")
            } else {
                let Some(segments) = expression_segments(field.object(), false) else {
                    return;
                };
                if segments
                    .first()
                    .is_none_or(|head| value_name_is_shadowed(head, state))
                {
                    return;
                }
                let Some(identity) = state.model.resolve_dotted(&segments) else {
                    return;
                };
                if !state
                    .model
                    .unaliased(identity)
                    .is_sdk("dart:async", "Future")
                    || !matches!(method, "wait" | "any")
                    || (method == "wait" && !discarded_await(node))
                    || (method == "any" && discarded_future(node, state))
                {
                    return;
                }
                None
            };
            (call.args(), replacement)
        };
        if !args.named().is_empty() || args.positional().len() != 1 {
            return;
        }
        let Some(argument) = args.positional().next() else {
            return;
        };
        let argument = argument.unparenthesized();
        let mut elements = match argument {
            Expr::List(list) => list.elements(),
            Expr::Set(set) => set.elements(),
            _ => return,
        };
        if elements.len() != 1 || !matches!(elements.next(), Some(CollectionElement::Expr(_))) {
            return;
        }
        let ty = state.infer(argument);
        if !ty.interface("dart:core", "List") && !ty.interface("dart:core", "Set") {
            return;
        }
        let message = match replacement {
            Some(replacement) => format!(
                "This singleton collection is unnecessary. Use `{replacement}` with its single element instead."
            ),
            None => "Use the single `Future` directly instead of wrapping it in `Future.wait` or `Future.any`."
                .to_owned(),
        };
        self.diagnostics.push(self.ctx.diagnostic(message, argument.span()));
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

fn discarded_await(expression: Expr<'_>) -> bool {
    let mut node = expression.syntax();
    while let Some(parent) = node.parent() {
        match Expr::cast(parent) {
            Some(Expr::Paren(_)) => node = parent,
            Some(Expr::Await(_)) => {
                node = parent;
                while let Some(parent) = node.parent() {
                    if matches!(Expr::cast(parent), Some(Expr::Paren(_))) {
                        node = parent;
                    } else {
                        return matches!(Stmt::cast(parent), Some(Stmt::Expr(_)));
                    }
                }
                return false;
            }
            _ => return false,
        }
    }
    false
}

fn discarded_future(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let mut node = expression.syntax();
    while let Some(parent) = node.parent() {
        match Expr::cast(parent) {
            Some(Expr::Paren(_) | Expr::Cascade(_)) => node = parent,
            Some(Expr::Await(_)) => return false,
            _ => {
                if matches!(Stmt::cast(parent), Some(Stmt::Expr(_))) {
                    return true;
                }
                if !matches!(state.expected(), Some(falcon_analyze::ResolvedType::Void))
                    || !matches!(FunctionBody::cast(parent), Some(FunctionBody::Arrow(_)))
                {
                    return false;
                }
                let Some(owner) = parent.parent() else {
                    return false;
                };
                let marker = FunctionDecl::cast(owner)
                    .map(|function| function.async_marker())
                    .or_else(|| LocalFuncDecl::cast(owner).map(|function| function.async_marker()))
                    .or_else(|| FuncExpr::cast(owner).map(|function| function.async_marker()))
                    .or_else(|| MethodDecl::cast(owner).map(|method| method.async_marker()));
                return marker == Some(AsyncMarker::Sync);
            }
        }
    }
    false
}
