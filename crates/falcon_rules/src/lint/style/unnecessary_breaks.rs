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

use falcon_analyze::{AnalyzeContext, Feature, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_switch_case};

#[derive(Default)]
pub struct UnnecessaryBreaks;

impl Rule for UnnecessaryBreaks {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.language_version().supports(Feature::Patterns) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_switch_case(&mut self, case: &SwitchCase) {
        if let [_, .., Stmt::Break(BreakStmt { label: None, span })] = case.body.as_slice() {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-breaks",
                "Unnecessary 'break' statement.",
                *span,
            ));
        }
        walk_switch_case(self, case);
    }
}
