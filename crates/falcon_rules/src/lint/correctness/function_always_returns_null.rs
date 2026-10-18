//! Flags value functions with a proven explicit null return on every path.
//! Bare returns and implicit fallthrough are deliberately not treated as proof.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use super::function_always_returns_same_value::{ReturnKind, prepare};

#[derive(Default)]
pub struct FunctionAlwaysReturnsNull;

impl Rule for FunctionAlwaysReturnsNull {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, ReturnKind::Null)
    }
}
