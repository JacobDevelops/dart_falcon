//! Flags an `else` clause whose body is an empty statement (`else ;`).
//!
//! An `else` immediately followed by a semicolon binds that empty statement as
//! the whole else branch, so the block meant to run instead executes
//! unconditionally right after the `if`. This is nearly always a stray semicolon
//! typed between `else` and the intended `{ ... }`. Remove the semicolon so the
//! following block becomes the else body, or delete the `else` entirely if no
//! alternative branch is needed.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidEmptyElse;

impl Rule for AvoidEmptyElse {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::If(if_stmt) = node
            && let Some(else_branch) = &if_stmt.else_branch
            && matches!(else_branch.as_ref(), Stmt::Empty(_))
        {
            let span = else_branch.span();
            self.diags.push(self.ctx.diagnostic(
                "avoid-empty-else",
                "Empty `else` clause — remove the `else` or give it a body",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
