//! Prefer EquatableMixin to directly extending package:equatable's Equatable.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferEquatableMixin;

impl Rule for PreferEquatableMixin {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let superclass = match declaration {
            // Existing mixins can have `on Equatable` constraints that require
            // the superclass before mixin application. Do not propose removing it.
            TopLevelDecl::Class(class) if class.with_clause().is_empty() => class.extends(),
            _ => None,
        };
        if let Some(superclass) = superclass
            && let Some(identities) = self.ctx.identities()
            && let ResolvedType::Interface { identity, .. } =
                SemanticModel::new(self.ctx.file_path(), self.ctx.parsed(), identities, self.ctx.types())
                    .resolve_type(superclass)
            && identities.is_package_declaration(&identity, "equatable", "Equatable")
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Prefer mixing in 'EquatableMixin' instead of extending 'Equatable'.",
                superclass.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
