//! Flags else branches after an if body that unconditionally returns or throws.
//!
//! The then branch must directly end in return, throw, or rethrow. Keep the
//! else body's scope when moving it after the if. Identical branches belong to
//! no-equal-then-else; a trailing bare return belongs to avoid-unnecessary-return
//! instead, because removing that return is the smaller simplification.
//! Only if statements directly inside blocks are checked, not nested unbraced
//! branches or else-if links, where moving statements can change control flow.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidRedundantElse;

impl Rule for AvoidRedundantElse {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(branch) = node
            && branch.syntax().parent().is_some_and(|parent| Block::cast(parent).is_some())
            && let Some(otherwise) = branch.else_branch()
        {
            let last = match branch.then_branch() {
                Stmt::Block(block) => block.stmts().last(),
                statement => Some(statement),
            };
            let exits = match last {
                Some(Stmt::Return(ret)) => ret.value().is_some()
                    || !super::avoid_unnecessary_return::at_function_end(node),
                Some(Stmt::Throw(_) | Stmt::Rethrow(_)) => true,
                _ => false,
            };
            let normalize = crate::lint::suspicious::no_equal_then_else::normalize;
            if exits && !normalize(self.ctx, branch.then_branch().span())
                .eq(normalize(self.ctx, otherwise.span())) {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-redundant-else",
                    "This else is redundant after an exiting branch; move its body after the if.",
                    otherwise.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }
}
}
