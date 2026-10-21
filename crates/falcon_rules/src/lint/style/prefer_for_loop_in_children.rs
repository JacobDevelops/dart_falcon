//! Prefers collection-for elements over eager map/toList chains in Flutter children.
//!
//! Direct `children` arguments are checked on resolved Flutter constructors.
//! Spreads are left to `unnecessary-to-list-in-spreads` to avoid duplicate reports.
//! The map receiver must be a proven
//! non-null SDK List, Set, or Iterable, with a single untyped positional parameter
//! and a synchronous arrow body. Named arguments, typed callbacks, async or block
//! bodies, null-aware calls, and custom collection APIs are left unchanged. The
//! suggestion retains eager evaluation and element order; no automatic fix is made.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferForLoopInChildren;

impl Rule for PreferForLoopInChildren {
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
        if !self.ctx.identities().is_some_and(|index| {
            index.is_package_declaration(&identity, "flutter", identity.name())
        }) {
            return;
        }
        for argument in args.named().filter(|arg| arg.name().name() == "children") {
            let expression = argument.value().unparenthesized();
            self.check(expression, state);
        }
    }
}

impl<'a> Collector<'a> {
    fn check(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(to_list) = expression.unparenthesized() else {
            return;
        };
        let Expr::Field(list_field) = to_list.callee().unparenthesized() else {
            return;
        };
        if list_field.is_null_safe()
            || list_field.field().name() != "toList"
            || !to_list.args().positional().is_empty()
            || !to_list.args().named().is_empty()
        {
            return;
        }
        let Expr::Call(map) = list_field.object().unparenthesized() else {
            return;
        };
        let Expr::Field(map_field) = map.callee().unparenthesized() else {
            return;
        };
        if map_field.is_null_safe()
            || map_field.field().name() != "map"
            || map.args().positional().len() != 1
            || !map.args().named().is_empty()
        {
            return;
        }
        let Some(Expr::FuncExpr(function)) =
            map.args().positional().next().map(Expr::unparenthesized)
        else {
            return;
        };
        let parameters = function.params().params();
        if function.async_marker() != AsyncMarker::Sync
            || !matches!(function.body(), FunctionBody::Arrow(_))
            || !function.type_params().is_empty()
            || parameters.len() != 1
            || !parameters.first().is_some_and(|parameter| {
                parameter.kind() == ParamKind::Positional && parameter.param_type().is_none()
            })
        {
            return;
        }
        let ResolvedType::Interface {
            identity,
            nullable: false,
            ..
        } = state.infer(map_field.object())
        else {
            return;
        };
        if !["List", "Set", "Iterable"]
            .iter()
            .any(|name| identity.is_sdk("dart:core", name))
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic("Use a collection-for element instead of 'map(...).toList()' in children.", expression.span()));
    }
}
