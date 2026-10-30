//! Disallows named local function declarations inside functions, methods and closures.
//!
//! Move helpers to a method or a top-level function instead. Anonymous callbacks
//! and function-valued local variables are not declarations and are not reported.
//!
//! Incompatible with `prefer-function-declarations-over-variables`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Program, Stmt};

#[derive(Default)]
pub struct AvoidLocalFunctions;

impl Rule for AvoidLocalFunctions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.diagnostics.push(self.ctx.diagnostic(
                "Avoid local functions. Move this declaration to a method or a top-level function.",
                function.name().span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
