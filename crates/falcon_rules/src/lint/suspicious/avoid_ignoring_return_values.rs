//! Flags a discarded call whose resolved return type is non-void.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidIgnoringReturnValues;

impl Rule for AvoidIgnoringReturnValues {
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

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::Expr(statement) = node else {
            return;
        };
        let Expr::Call(call) = statement.expr().unparenthesized() else {
            return;
        };
        let Some(return_type) =
            state.call_return_type(call.callee(), call.type_args(), call.args(), None)
        else {
            return;
        };
        if matches!(return_type, ResolvedType::Unknown | ResolvedType::Dynamic)
            || state.model.void_context(&return_type)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("The return value is not being used.", call.span()));
    }
}
