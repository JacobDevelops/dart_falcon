//! Reports a single-use local used only to initialize the next local.
//! Both declarations must be eager and inferred: transferring the initializer
//! then preserves evaluation order and its type-inference context.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryLocalVariable;

impl Rule for AvoidUnnecessaryLocalVariable {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let references = local_declarations(program);
        let mut uses = HashMap::new();
        for id in references.values() {
            *uses.entry(*id).or_insert(0usize) += 1;
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            references,
            uses,
            mutated: ctx.mutated_declarations(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    uses: HashMap<NodeId, usize>,
    mutated: &'a HashSet<NodeId>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_block(&mut self, block: Block<'a>) -> StructuralTraversal {
        for (previous, next) in block.stmts().zip(block.stmts().skip(1)) {
            if let (Stmt::LocalVar(first), Stmt::LocalVar(second)) = (previous, next)
                && !first.is_late() && !second.is_late()
                && first.var_type().is_none() && second.var_type().is_none()
                && first.keyword() != VarKeyword::Const && second.keyword() != VarKeyword::Const
                && first.declarators().count() == 1 && second.declarators().count() == 1
                && let Some(variable) = first.declarators().next()
                && variable.initializer().is_some()
                && let Some(alias) = second.declarators().next()
                && let Some(Expr::Ident(name)) = alias.initializer().map(Expr::unparenthesized)
            {
                let id = variable.name().syntax().id();
                if self.references.get(&name.span().start) == Some(&id)
                    && self.uses.get(&id) == Some(&1)
                    && !self.mutated.contains(&id)
                {
                    self.diagnostics.push(self.ctx.diagnostic("Initialize the next local directly instead of introducing this single-use alias.", variable.span()));
                }
            }
        }
        StructuralTraversal::Continue
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
