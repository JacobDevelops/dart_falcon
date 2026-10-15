//! Flags a discarded call whose resolved return type is non-void.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidIgnoringReturnValues;

impl Rule for AvoidIgnoringReturnValues {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::Expr(statement) = node else {
            return;
        };
        let Expr::Call {
            callee,
            type_args,
            args,
            span,
            ..
        } = statement.expr.unparenthesized()
        else {
            return;
        };
        let Some(return_type) = state.call_return_type(callee, type_args, args, None) else {
            return;
        };
        if matches!(return_type, ResolvedType::Unknown | ResolvedType::Dynamic)
            || state.model.void_context(&return_type)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-ignoring-return-values",
            "The return value is not being used",
            *span,
        ));
    }
}
