//! Flags uninitialized variables declared without a type annotation.
//!
//! When a declaration has no initializer, there is no value for the analyzer to
//! infer a type from, so `var x;` leaves the variable as `dynamic` — silently
//! opting out of static checking on every later use. Writing `int x;` instead
//! documents intent and restores type safety at no cost. The rule fires only on
//! declarators that are both untyped and uninitialized; a declaration is exempt
//! as soon as it carries a type annotation. It covers local variables, fields,
//! and top-level variables.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferTypingUninitializedVariables;

impl Rule for PreferTypingUninitializedVariables {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, has_type: bool, declarators: AstChildren<'a, VarDeclarator<'a>>) {
        if has_type {
            return;
        }
        for decl in declarators {
            if decl.initializer().is_none() {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-typing-uninitialized-variables",
                    "Prefer typing uninitialized variables and fields.".to_string(),
                    decl.name().span(),
                ));
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_program(&mut self, node: Program<'a>) -> StructuralTraversal {
        for decl in node.declarations() {
            if let TopLevelDecl::Variable(var) = decl {
                self.check(var.var_type().is_some(), var.declarators());
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        self.check(node.field_type().is_some(), node.declarators());
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, local: LocalVarDecl<'a>) -> StructuralTraversal {
        self.check(local.var_type().is_some(), local.declarators());
        StructuralTraversal::Continue
    }
}
}
