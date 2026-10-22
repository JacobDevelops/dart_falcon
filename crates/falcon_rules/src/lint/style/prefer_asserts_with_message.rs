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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

const MESSAGE: &str = "Missing a message in an assert.";

#[derive(Default)]
pub struct PreferAssertsWithMessage;

impl Rule for PreferAssertsWithMessage {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        for initializer in node.initializers {
            if let ConstructorInitializer::Assert(assertion) = initializer
                && assertion.message().is_none()
            {
                self.diags.push(self.ctx.diagnostic(MESSAGE, assertion.span()));
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Assert(assertion) = node
            && assertion.message().is_none()
        {
            self.diags.push(self.ctx.diagnostic(MESSAGE, assertion.span()));
        }
        StructuralTraversal::Continue
    }
}
}
