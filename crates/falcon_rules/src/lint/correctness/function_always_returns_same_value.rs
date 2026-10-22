//! Warns when distinct return paths in a function produce the same scalar constant.
//! Only synchronous value functions and scalar constants are considered. Unsupported
//! control flow (including loops, switches and exception handling) fails closed.

use crate::lint::support::constant_returns::{ReturnKind, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct FunctionAlwaysReturnsSameValue;

impl Rule for FunctionAlwaysReturnsSameValue {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, ReturnKind::NonNull)
    }
}

