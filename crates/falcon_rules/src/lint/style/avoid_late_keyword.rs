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
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidLateKeyword;

impl Rule for AvoidLateKeyword {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
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
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Variable(var) = node
            && var.is_late
        {
            self.diags.push(self.ctx.diagnostic("avoid-late-keyword", "Avoid using the late keyword — use nullable types or initialize immediately instead", var.span));
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        if node.is_late {
            self.diags.push(self.ctx.diagnostic("avoid-late-keyword", "Avoid using the late keyword — use nullable types or initialize immediately instead", node.span));
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, local: &LocalVarDecl) -> StructuralTraversal {
        if local.is_late {
            self.diags.push(self.ctx.diagnostic("avoid-late-keyword", "Avoid using the late keyword — use nullable types or initialize immediately instead", local.full_range()));
        }
        StructuralTraversal::Continue
    }
}
}
