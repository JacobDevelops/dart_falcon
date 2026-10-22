//! Prefer explicit types over `var` in ordinary variable declarations.
//!
//! Checks local, field and top-level `var` declarations, regardless of initializer.
//! Explicit types, final and const declarations are not checked. Destructuring
//! patterns and implicit for-in binding forms are excluded.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferTypeOverVar;

impl Rule for PreferTypeOverVar {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
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

impl Collector<'_> {
    fn check(&mut self, declarations: AstChildren<'_, VarDeclarator<'_>>) {
        for declaration in declarations {
            self.diags.push(self.ctx.diagnostic("This variable uses `var`; replace it with an explicit type annotation.", declaration.name().span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        if node.keyword() == VarKeyword::Var && node.var_type().is_none() { self.check(node.declarators()); }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.keyword() == VarKeyword::Var && node.field_type().is_none() { self.check(node.declarators()); }
        StructuralTraversal::Continue
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(node) = node
            && node.keyword() == VarKeyword::Var && node.var_type().is_none() {
            self.check(node.declarators());
        }
        StructuralTraversal::Continue
    }
}
}
