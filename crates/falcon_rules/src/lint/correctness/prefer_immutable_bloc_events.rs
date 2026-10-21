//! Flags Bloc event classes without an explicit @immutable annotation.
//!
//! Immutable models keep state changes predictable and discourage accidental mutation.
//! Identifies roots from the generic arguments of package-confirmed Bloc
//! superclasses, then checks those roots and their resolved subclasses.
//! Unrelated name suffixes and unresolved model types are ignored.
//! Each class must carry @immutable or @Immutable itself; inheritance is not enough.
//! Roots and declarations must be in the current file; separate part files are not joined.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::bloc::{ModelKind, ModelRequirement, prepare_models};

#[derive(Default)]
pub struct PreferImmutableBlocEvents;

impl Rule for PreferImmutableBlocEvents {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_models(
            program,
            ctx,
            ModelKind::Event,
            ModelRequirement::Immutable,
        )
    }
}
