use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Program;

use crate::AnalyzeContext;
use crate::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::structural_scope::{self, StructuralRuleVisitor};

pub enum RulePreparation<'a> {
    Diagnostics(Vec<Diagnostic>),
    Semantic(Box<dyn SemanticRuleVisitor<'a> + 'a>),
    Structural(Box<dyn StructuralRuleVisitor<'a> + 'a>),
}

/// Trait implemented by every lint rule.
///
/// Rule instances are immutable; diagnostics are local per file.
/// Thread safety: implementors MUST NOT use mutable self state.
pub trait Rule: Send + Sync {
    /// Returns collector output; use a registry or [`RuleAnalysis`] to assign ids.
    fn prepare<'a>(&self, program: Program<'a>, ctx: &'a AnalyzeContext<'a>)
    -> RulePreparation<'a>;
}

/// Standalone analysis with the same fixed lifecycle as registry execution.
///
/// Implement [`Rule::prepare`] instead; the blanket implementation keeps direct and
/// registry analysis from diverging through an overridden entry point.
/// The name is the canonical id supplied by the caller's rule declaration.
pub trait RuleAnalysis: Rule {
    fn analyze<'a>(
        &self,
        name: &'static str,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Vec<Diagnostic>;
}

impl<T: Rule + ?Sized> RuleAnalysis for T {
    fn analyze<'a>(
        &self,
        name: &'static str,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = match self.prepare(program, ctx) {
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
        };
        let config_errors = ctx.take_config_errors();
        if !config_errors.is_empty() {
            return config_errors;
        }
        for diagnostic in &mut diagnostics {
            diagnostic.rule = name;
        }
        diagnostics
    }
}
