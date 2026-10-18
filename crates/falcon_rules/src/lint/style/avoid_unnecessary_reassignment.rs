//! Reports an eager local initializer immediately overwritten without a read.
//! An explicit declared type preserves inference when the assignment moves.
//! Only effect-free primitive/string initializers are discardable. Branching
//! if/else overwrites are deliberately excluded without control-flow proof.

use std::collections::HashMap;

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryReassignment;

impl Rule for AvoidUnnecessaryReassignment {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            references: local_declarations(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_block(&mut self, block: Block<'a>) -> StructuralTraversal {
        for (previous, next) in block.stmts().zip(block.stmts().skip(1)) {
            let (Stmt::LocalVar(declaration), Stmt::Expr(statement)) = (previous, next) else { continue; };
            if declaration.is_late() || declaration.keyword() != VarKeyword::Var
                || declaration.var_type().is_none()
                || declaration.declarators().count() != 1 { continue; }
            let Some(variable) = declaration.declarators().next() else { continue; };
            let Some(initializer) = variable.initializer().map(Expr::unparenthesized) else { continue; };
            let discardable = matches!(initializer, Expr::IntLit(_) | Expr::DoubleLit(_)
                | Expr::BoolLit(_) | Expr::NullLit(_))
                || matches!(initializer, Expr::StringLit(string) if string.interpolations().next().is_none());
            if !discardable { continue; }
            let Expr::Assign(assign) = statement.expr().unparenthesized() else { continue; };
            if assign.op() != AssignOp::Eq { continue; }
            let Expr::Ident(target) = assign.target().unparenthesized() else { continue; };
            let id = variable.name().syntax().id();
            if self.references.get(&target.span().start) != Some(&id) { continue; }
            let rhs = assign.value().span();
            // Includes reads captured by a closure in the new initializer.
            if self.references.iter().any(|(offset, declaration)|
                *declaration == id && rhs.start <= *offset && *offset < rhs.end) { continue; }
            self.diagnostics.push(self.ctx.diagnostic("avoid-unnecessary-reassignment",
                "Initialize this local with the assigned value instead of overwriting an unused initial value.", assign.span()));
        }
        StructuralTraversal::Continue
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
