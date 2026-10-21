//! Flags Bloc state classes without a sealed or final modifier.
//!
//! Closed hierarchies make exhaustive handling reliable.
//! Identifies roots from the generic arguments of package-confirmed Bloc
//! or Cubit superclasses, then checks those roots and their resolved subclasses.
//! Unrelated name suffixes and unresolved model types are ignored.
//! Both sealed and final satisfy the rule.
//! Roots and declarations must be in the current file; separate part files are not joined.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::bloc::{ModelKind, ModelRequirement, prepare_models};

#[derive(Default)]
pub struct PreferSealedBlocState;

impl Rule for PreferSealedBlocState {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_models(
            program,
            ctx,
            ModelKind::State,
            ModelRequirement::Sealed,
        )
    }
}
