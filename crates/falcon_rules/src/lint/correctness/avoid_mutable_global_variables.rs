//! Disallow mutable top-level variables.
//!
//! Flags a top-level variable that is neither `const` nor `final`. A mutable
//! global can be reassigned from anywhere in the program, so its value at any
//! moment depends on execution order that no local reading of the code reveals —
//! a recurring source of subtle bugs and flaky, order-dependent tests. Declare
//! it `const` or `final`; when a value genuinely must change over time, hold it
//! in an object with a clear owner rather than a free-floating global.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMutableGlobalVariables;

impl Rule for AvoidMutableGlobalVariables {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(var_decl) = decl
            && var_decl.keyword() == VarKeyword::Var
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-mutable-global-variables",
                "Avoid mutable global variables. Use const or final instead.",
                var_decl.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
