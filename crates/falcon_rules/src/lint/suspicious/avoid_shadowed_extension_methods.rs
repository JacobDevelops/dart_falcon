//! Flags extension instance methods and accessors hidden by their target's members.
//!
//! Instance members take precedence over implicit extension invocation, so a
//! colliding extension member needs an explicit extension override to be called.
//! Rename it instead. The target is resolved by declaration identity, including
//! aliases and inherited members; getters, fields and setters can also hide a
//! method. Static extension declarations and other extensions do not compete.
//! Unresolved, dynamic and nullable targets are skipped; type parameters are
//! checked only when their resolved non-nullable bound proves the collision.
//! This checks extension declarations, not local variables inside extensions.
//! SDK members absent from the precise instance-signature inventory are skipped.

use std::borrow::Cow;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_shadowing::has_instance_member;

#[derive(Default)]
pub struct AvoidShadowedExtensionMethods;

impl Rule for AvoidShadowedExtensionMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let signatures = ctx
            .signatures()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(SignatureIndex::from_program(program, &model)));
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            receiver: ResolvedType::Unknown,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: Cow<'a, SignatureIndex>,
    receiver: ResolvedType,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        let TopLevelDecl::Extension(extension) = node else {
            return StructuralTraversal::SkipChildren;
        };
        let mut parameters = TypeParameterScope::default();
        parameters.push(extension.type_params(), &self.model);
        self.receiver = self.model.resolve_type_in(extension.on_type(), &parameters);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        let name = match node {
            ClassMember::Method(method) if !method.is_static() => method.name(),
            ClassMember::Getter(getter) if !getter.is_static() => getter.name(),
            ClassMember::Setter(setter) if !setter.is_static() => setter.name(),
            _ => return StructuralTraversal::SkipChildren,
        };
        if has_instance_member(&self.model, &self.signatures, &self.receiver, name.name()) {
            self.diagnostics.push(self.ctx.diagnostic(
                "The extension target already has this member. Rename the extension member.",
                name.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}
