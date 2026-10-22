//! Exception and Error payloads should not change after construction.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidNonFinalExceptionClassFields;

impl Rule for AvoidNonFinalExceptionClassFields {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, name: Identifier<'_>) {
        self.diagnostics.push(self.ctx.diagnostic("Make exception instance fields `final`.", name.span()));
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        let Some(declaration) = crate::lint::inherited_members::inheriting_declaration(node) else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(identity) = self.model.resolve_name(declaration.name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        let ty = ResolvedType::new_interface(identity, Vec::new().into(), false, false);
        if !["Exception", "Error"].iter().any(|name| {
            self.signatures.interface_check_sdk(&ty, "dart:core", name, &self.model) == Truth::Yes
        }) {
            return StructuralTraversal::SkipChildren;
        }
        for parameter in (EffectiveMembers {
            declared: declaration.members.clone(), primary: declaration.primary_constructor,
            representation: false,
        }).declaring_params() {
            if !parameter.is_final() { self.report(parameter.name()); }
        }
        for member in declaration.members {
            if let ClassMember::Field(field) = member
                && !field.is_static() && field.keyword() == VarKeyword::Var
            {
                for declarator in field.declarators() { self.report(declarator.name()); }
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
