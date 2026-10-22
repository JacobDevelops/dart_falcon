//! Flags BuildContext use after an asynchronous gap without a mounted guard.
//!
//! A mounted guard must check the same `BuildContext` used after the gap.
//! Function bodies are checked independently; callbacks passed to SDK async APIs
//! that guarantee a mounted context are exempt.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
use crate::lint::declarations::in_test_directory as is_test_file;
use crate::lint::support::async_semantics as semantics;


#[derive(Default)]
pub struct UseBuildContextSynchronously;

impl Rule for UseBuildContextSynchronously {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if is_test_file(ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let Some(semantics) = semantics::prepare(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Semantic(Box::new(semantics))
    }
}



