//! Flags declared instance fields omitted from a statically understood Equatable props list.
//!
//! Only resolved transitive package:equatable Equatable/EquatableMixin subtypes inheriting
//! Equatable's concrete equality are checked; classes replacing equality are excluded.
//! Instance storage includes private and
//! primary-constructor fields, but not static, abstract or external fields or computed
//! getters. Arrow getters and single-return blocks with literal lists are supported,
//! including this.field, super.props spreads and cascaded addAll calls with known lists.
//! Unknown expressions, collection control flow, and unknown spreads are left silent.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct ListAllEquatableFields;

impl Rule for ListAllEquatableFields {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::equatable::prepare(ctx, super::equatable::Check::Fields)
    }
}
