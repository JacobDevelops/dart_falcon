//! Merge consecutive Flutter State.setState statements in the same block.
//! Intervening statements and control-flow boundaries are not collapsible runs.

use std::collections::HashSet;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::correctness::flutter_state_rules::is_setstate;

#[derive(Default)]
pub struct PreferSingleSetstate;

impl Rule for PreferSingleSetstate {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            calls: HashSet::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    calls: HashSet<NodeId>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Call(call) = expression
            && is_setstate(call, state)
        {
            self.calls.insert(call.syntax().id());
        }
    }

    fn visit_block_end(&mut self, block: Block<'a>, _state: &SemanticState<'a>) {
        let mut previous = false;
        for statement in block.stmts() {
            let current = match statement {
                Stmt::Expr(statement) => match statement.expr().unparenthesized() {
                    Expr::Call(call) => self.calls.contains(&call.syntax().id()),
                    _ => false,
                },
                _ => false,
            };
            if previous && current {
                self.diagnostics.push(self.ctx.diagnostic(
                    "prefer-single-setstate",
                    "Merge this setState callback into the preceding setState call.",
                    statement.span(),
                ));
            }
            previous = current;
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
