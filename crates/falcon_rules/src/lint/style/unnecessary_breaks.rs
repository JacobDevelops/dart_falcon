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
//! falcon has no language-version model, so the rule also fires in libraries
//! whose language version predates Dart 3.0, where the official lint is silent
//! because those `break`s prevent fall-through.
//!
//! Adopted from the official `unnecessary_breaks` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct UnnecessaryBreaks;

impl Rule for UnnecessaryBreaks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Switch(switch) = node {
            for case in &switch.cases {
                if let [_, .., Stmt::Break(BreakStmt { label: None, span })] = case.body.as_slice()
                {
                    self.diags.push(self.ctx.diagnostic(
                        "unnecessary-breaks",
                        "Unnecessary 'break' statement.",
                        *span,
                    ));
                }
            }
        }
        walk_stmt(self, node);
    }
}
