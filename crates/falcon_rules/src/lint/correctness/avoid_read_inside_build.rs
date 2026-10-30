//! Flags non-listening Provider reads directly inside Flutter widget builds.
//!
//! A read does not rebuild the widget when its value changes. This checks
//! package:provider context extensions and `Provider.of(listen: false)` on a
//! Flutter `BuildContext`. Nested callbacks are skipped: their execution phase
//! depends on the receiving API, not their lexical position. `BuildWithChild`
//! overrides on nested's single-child widgets are also build entry points;
//! didChangeDependencies permits subscriptions but is not a build.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
use crate::lint::support::provider_access::{Check, prepare};

#[derive(Default)]
pub struct AvoidReadInsideBuild;

impl Rule for AvoidReadInsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Check::ReadInsideBuild)
    }
}

