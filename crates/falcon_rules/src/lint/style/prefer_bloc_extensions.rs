//! Prefer context.read/watch over package:flutter_bloc's provider lookups.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, QualifiedName, Rule, RulePreparation, expression_segments};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
#[derive(Default)]
pub struct PreferBlocExtensions;
impl Rule for PreferBlocExtensions {
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
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else {
            return;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        if field.field().name() != "of"
            || field.is_null_safe()
            || call.args().positional().len() != 1
        {
            return;
        }
        if call.args().named().any(|arg| {
            arg.name().name() != "listen"
                || !matches!(arg.value().unparenthesized(), Expr::BoolLit(_))
        }) {
            return;
        }
        let Some(segments) = expression_segments(field.object(), false) else {
            return;
        };
        if crate::lint::flutter::value_name_is_shadowed(segments[0], state) {
            return;
        }
        let Some(name) = QualifiedName::from_segments(&segments) else {
            return;
        };
        let Some(identity) = state.model.resolve_name(name) else {
            return;
        };
        if !["BlocProvider", "RepositoryProvider"].iter().any(|name| {
            self.ctx
                .identities()
                .is_some_and(|index| index.is_package_declaration(&identity, "flutter_bloc", name))
        }) {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("Use context.read or context.watch instead of a provider's of method.", field.field().span()));
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
