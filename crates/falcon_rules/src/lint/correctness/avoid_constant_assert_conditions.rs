//! Flags assertions whose Boolean condition is statically constant.
//! Remove an always-passing assertion or fix an always-failing condition.

use super::avoid_constant_conditions::constant_value;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ConstantValue, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidConstantAssertConditions;

impl Rule for AvoidConstantAssertConditions {
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

impl<'a> Collector<'a> {
    fn check(&mut self, condition: Expr<'a>, state: &SemanticState<'a>) {
        if let Some(ConstantValue::Bool(value)) = constant_value(condition, state, self.ctx) {
            let message = if value {
                "Assertion always passes; remove it or assert a runtime condition."
            } else {
                "Assertion always fails; fix the condition or replace it with an explicit failure."
            };
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-constant-assert-conditions",
                message,
                condition.span(),
            ));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Assert(node) = node {
            self.check(node.condition(), state);
        }
    }

    fn visit_constructor_ref(&mut self, node: ConstructorRef<'a>, state: &SemanticState<'a>) {
        for initializer in node.initializers {
            if let ConstructorInitializer::Assert(node) = initializer {
                self.check(node.condition(), state);
            }
        }
    }
}
