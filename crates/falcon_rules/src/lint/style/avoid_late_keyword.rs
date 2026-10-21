//! Flags use of the `late` keyword on variables and fields.
//!
//! `late` defers initialization and moves the "is it set?" check from compile
//! time to runtime: reading a `late` variable before it is assigned throws a
//! `LateInitializationError` rather than failing to compile. Preferring eagerly
//! initialized or nullable declarations keeps that guarantee static and the
//! failure mode visible. Top-level variables, instance fields, and local
//! declarations inside any function body are all checked.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidLateKeyword;

impl Rule for AvoidLateKeyword {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(var) = node
            && var.is_late()
        {
            self.diags.push(self.ctx.diagnostic("Avoid using the late keyword — use nullable types or initialize immediately instead", var.span()));
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.is_late() {
            self.diags.push(self.ctx.diagnostic("Avoid using the late keyword — use nullable types or initialize immediately instead", node.span()));
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, local: LocalVarDecl<'a>) -> StructuralTraversal {
        if local.is_late() {
            self.diags.push(self.ctx.diagnostic("Avoid using the late keyword — use nullable types or initialize immediately instead", local.full_span()));
        }
        StructuralTraversal::Continue
    }
}
}
