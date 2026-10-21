//! Flags switch statements and expressions with a statically constant subject.
//! Replace the switch with the matching branch.

use super::avoid_constant_conditions::constant_value;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidConstantSwitches;

impl Rule for AvoidConstantSwitches {
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
    fn check(&mut self, subject: Expr<'a>, state: &SemanticState<'a>) {
        if constant_value(subject, state, self.ctx).is_some() {
            self.diagnostics.push(self.ctx.diagnostic("Switch subject is statically constant; replace the switch with the matching branch.", subject.span()));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Switch(node) = node {
            self.check(node.subject(), state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Switch(node) = node {
            self.check(node.subject(), state);
        }
    }
}
