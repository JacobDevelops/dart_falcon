use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::AnalyzeContext;
use crate::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::structural_scope::{self, StructuralRuleVisitor};

pub enum RulePreparation<'a> {
    Diagnostics(Vec<Diagnostic>),
    Semantic(Box<dyn SemanticRuleVisitor + 'a>),
    Structural(Box<dyn StructuralRuleVisitor + 'a>),
}

/// Trait implemented by every lint rule.
///
/// Rule instances are immutable; diagnostics are local per file.
/// Thread safety: implementors MUST NOT use mutable self state.
pub trait Rule: Send + Sync {
    fn prepare<'a>(&self, program: &'a Program, ctx: &'a AnalyzeContext<'a>)
    -> RulePreparation<'a>;
}

/// Standalone analysis with the same fixed lifecycle as registry execution.
///
/// Implement [`Rule::prepare`] instead; the blanket implementation keeps direct and
/// registry analysis from diverging through an overridden entry point.
pub trait RuleAnalysis: Rule {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>;
}

impl<T: Rule + ?Sized> RuleAnalysis for T {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        match self.prepare(program, ctx) {
            RulePreparation::Diagnostics(diagnostics) => diagnostics,
            RulePreparation::Semantic(mut visitor) => {
                if let Some(state) = SemanticState::new(program, ctx) {
                    visit_program(&mut *visitor, program, state);
                }
                visitor.finish()
            }
            RulePreparation::Structural(visitor) => {
                structural_scope::visit_program(program, vec![visitor])
                    .into_iter()
                    .flatten()
                    .collect()
            }
        }
    }
}
