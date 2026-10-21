//! Avoids Flutter's non-const `Border.all` factory. Use
//! `Border.fromBorderSide(const BorderSide(...))` when the side is constant.
//! Only invocations whose arguments can all be constant qualify.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidBorderAll;

impl Rule for AvoidBorderAll {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            deferred: crate::lint::const_eval::deferred_prefixes(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    deferred: Vec<&'a str>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, "all", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "Border") {
            return;
        }
        if !args
            .positional()
            .chain(args.named().map(|arg| arg.value()))
            .all(|value| crate::lint::const_eval::constant_in_context(value, &self.deferred, state))
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("Prefer Border.fromBorderSide with a const BorderSide instead of Border.all.", crate::lint::flutter::constructor_span(node)));
    }
}
