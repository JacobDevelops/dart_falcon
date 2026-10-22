//! Flags a trailing `break` in a switch case that already ends implicitly.
//!
//! Since Dart 3.0, a non-empty `case` body never falls through, so an
//! unlabeled `break;` as its last statement does nothing. A case whose only
//! statement is `break;` is left alone: without it the case would be empty and
//! share the next case's body.
//!
//! ```dart
//! switch (x) {
//!   case 1:
//!     log();
//!     break; // flagged
//!   case 2:
//!     break; // ok: keeps the case from sharing the next body
//!   default:
//!     log();
//! }
//! ```
//!
//! Like the official lint, the rule is silent in libraries whose language
//! version predates Dart 3.0, where those `break`s prevent fall-through.
//!
//! Adopted from the official `unnecessary_breaks` lint.

use falcon_analyze::{
    AnalyzeContext, Feature, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryBreaks;

impl Rule for UnnecessaryBreaks {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.language_version().supports(Feature::Patterns) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, 'a> {
    fn visit_switch_case(&mut self, case: SwitchCase<'a>) -> StructuralTraversal {
        let mut body = case.body();
        if let Some(Stmt::Break(statement)) = body.next_back()
            && statement.label().is_none()
            && body.next().is_some()
        {
            self.diags.push(self.ctx.diagnostic(
                "Remove the unnecessary `break` statement.",
                statement.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
