//! Flags standalone nested blocks that introduce no lexical declarations.
//!
//! Control-flow and function bodies retain their braces. A block containing a
//! variable, pattern, or local function declaration retains its lexical scope.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryBlock;

impl Rule for AvoidUnnecessaryBlock {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Block(block) = node
            && block.syntax().parent().is_some_and(|parent| parent.kind() == NodeKind::Block)
            && block.stmts().all(|statement| matches!(statement,
                Stmt::Block(_) | Stmt::If(_) | Stmt::For(_) | Stmt::While(_)
                | Stmt::DoWhile(_) | Stmt::Switch(_) | Stmt::TryCatch(_)
                | Stmt::Return(_) | Stmt::Throw(_) | Stmt::Break(_)
                | Stmt::Continue(_) | Stmt::Assert(_) | Stmt::Yield(_)
                | Stmt::Expr(_) | Stmt::Rethrow(_) | Stmt::Empty(_)
            ))
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Remove this unnecessary block without changing the enclosed statements.",
                block.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
