//! Flags concrete Equatable subclasses that do not declare an instance props property.
//!
//! A concrete subclass must explicitly define which values participate in equality, even
//! when a parent already supplies props. An instance getter, field or declaring constructor
//! parameter named props satisfies the rule. Abstract/sealed bases are exempt. Only resolved
//! transitive package:equatable subtypes inheriting Equatable's concrete equality are checked;
//! interface-only implementations, classes replacing equality, unrelated same-named types
//! and unknown types are ignored.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AddEquatableProps;

impl Rule for AddEquatableProps {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::equatable::prepare(ctx, crate::lint::support::equatable::Check::Props)
    }
}
