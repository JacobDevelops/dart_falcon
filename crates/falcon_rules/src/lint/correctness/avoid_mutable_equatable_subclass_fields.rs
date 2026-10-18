//! Flags mutable instance fields on subclasses inheriting Equatable equality.
//!
//! Changing value-object state can invalidate hash-based collection membership. This checks
//! declared fields, including private, late and primary-constructor fields, only when the
//! resolved inherited equality implementation belongs to package:equatable Equatable or
//! EquatableMixin. Static, final, abstract and external fields, interface-only implementations,
//! unresolved inheritance and classes replacing equality themselves are excluded.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidMutableEquatableSubclassFields;

impl Rule for AvoidMutableEquatableSubclassFields {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::equatable::prepare(ctx, super::equatable::Check::Mutable)
    }
}
