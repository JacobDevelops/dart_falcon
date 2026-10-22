//! Reports unconditional break, continue, return or throw in a for-loop body.
//!
//! Branches, nested loops, try/finally, and labeled jumps are not flattened:
//! they may change which iteration or construct an exit interrupts.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnconditionalBreak;

impl Rule for AvoidUnconditionalBreak {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>) -> StructuralTraversal {
        let body = match statement {
            Stmt::For(node) => Some(node.body()),
            _ => None,
        };
        if let Some(span) = body.and_then(unconditional_break) {
            self.diags.push(self.ctx.diagnostic(
                "This statement unconditionally interrupts every loop iteration; make the exit conditional.",
                span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn unconditional_break(body: Stmt<'_>) -> Option<Span> {
    match body {
        Stmt::Break(node) if node.label().is_none() => Some(node.span()),
        Stmt::Continue(node) if node.label().is_none() => Some(node.span()),
        Stmt::Return(node) => Some(node.span()),
        Stmt::Throw(node) => Some(node.span()),
        Stmt::Block(block) => {
            for statement in block.stmts() {
                match statement {
                    Stmt::Break(node) if node.label().is_none() => return Some(node.span()),
                    Stmt::Continue(node) if node.label().is_none() => return Some(node.span()),
                    Stmt::Return(node) => return Some(node.span()),
                    Stmt::Throw(node) => return Some(node.span()),
                    Stmt::Empty(_)
                    | Stmt::Expr(_)
                    | Stmt::LocalVar(_)
                    | Stmt::LocalFunc(_)
                    | Stmt::Assert(_) => {}
                    _ => return None,
                }
            }
            None
        }
        _ => None,
    }
}
