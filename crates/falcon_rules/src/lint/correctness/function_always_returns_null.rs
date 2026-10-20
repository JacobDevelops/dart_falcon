//! Flags value functions with a proven explicit null return on every path.
//! Bare returns and implicit fallthrough are deliberately not treated as proof.
//! Only synchronous value functions are considered; getters, setters,
//! `@override`-annotated methods and callbacks are excluded. Unsupported control
//! flow (including loops, switches and exception handling) fails closed.

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
