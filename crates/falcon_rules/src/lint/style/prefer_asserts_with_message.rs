//! Flags `assert` statements and assert initializers that have no message.
//!
//! When an assertion fails, the message is the only hint about why the
//! invariant mattered. Supply one as the second argument:
//!
//! ```dart
//! assert(count >= 0);                                // flagged
//! assert(count >= 0, 'count must not be negative');  // ok
//! ```
//!
//! Adopted from the official `prefer_asserts_with_message` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

const RULE: &str = "prefer-asserts-with-message";
const MESSAGE: &str = "Missing a message in an assert.";

#[derive(Default)]
pub struct PreferAssertsWithMessage;

impl Rule for PreferAssertsWithMessage {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
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
    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        for initializer in node.initializers {
            if let ConstructorInitializer::Assert {
                message: None,
                span,
                ..
            } = initializer
            {
                self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, *span));
            }
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Assert(AssertStmt {
            message: None,
            span,
            ..
        }) = node
        {
            self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, *span));
        }
        walk_stmt(self, node);
    }
}
