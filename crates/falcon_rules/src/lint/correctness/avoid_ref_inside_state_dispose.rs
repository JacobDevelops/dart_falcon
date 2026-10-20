//! Rejects Riverpod widget references used during State.dispose.

use crate::lint::function_bodies::contains;
use crate::lint::riverpod::is_type;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidRefInsideStateDispose;

impl Rule for AvoidRefInsideStateDispose {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut disposals = Vec::new();
        for declaration in program.declarations() {
            let members = match declaration {
                TopLevelDecl::Class(class) => class.members(),
                TopLevelDecl::MixinClass(class) => class.members(),
                TopLevelDecl::Mixin(mixin) => mixin.members(),
                _ => continue,
            };
            for member in members {
                if let ClassMember::Method(method) = member
                    && method.name().name() == "dispose"
                    && !method.is_static()
                    && let Some(body) = method.body()
                {
                    disposals.push(body.span());
                }
            }
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            disposals,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    disposals: Vec<Span>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(expression, Expr::Ident(_) | Expr::Field(_))
            || !self
                .disposals
                .iter()
                .any(|body| contains(body, &expression.span()))
            || !state
                .enclosing_type
                .as_ref()
                .is_some_and(|ty| is_type(self.ctx, state, ty, &["ConsumerState"]))
            || !is_type(self.ctx, state, &state.infer(expression), &["WidgetRef"])
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-ref-inside-state-dispose",
            "Do not use a widget ref during dispose; cache the required resource before disposal.",
            expression.span(),
        ));
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
