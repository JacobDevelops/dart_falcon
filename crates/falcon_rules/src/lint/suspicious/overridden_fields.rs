//! Flags instance fields that redeclare an inherited field.
//!
//! Adopted from the official `overridden_fields` lint.
//! A field is reported when its inherited concrete super implementation is a
//! field — the first concrete getter-providing member of the superclass chain
//! and mixin applications, or for a mixin the member of its `on` types'
//! interfaces (`Object` without one). Fields reached only through `implements`,
//! abstract members of superclasses, and fields shadowed by a nearer getter or
//! method are not reported; covariance does not matter. Extension types
//! inherit no concrete members.
//! Declaring `var`/`final` primary constructor parameters are instance fields
//! too; forwarding formals and ordinary header parameters are not declarations.

use falcon_analyze::{
    AnalyzeContext, MemberKind, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::inherited_members::{inherited_concrete_member, inheriting_declaration};

#[derive(Default)]
pub struct OverriddenFields;

impl Rule for OverriddenFields {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let Some(declaration) = inheriting_declaration(decl) else {
            return StructuralTraversal::SkipChildren;
        };
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let diags = &mut self.diags;
        let Some(owner) = model.resolve_name(declaration.name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        for parameter in (EffectiveMembers {
            declared: declaration.members.clone(),
            primary: declaration.primary_constructor,
            representation: false,
        }
        .declaring_params())
        {
            if inherited_concrete_member(
                model,
                signatures,
                &owner,
                parameter.name().name(),
                &declaration.source,
            )
            .is_some_and(|inherited| matches!(inherited.kind, MemberKind::Field { .. }))
            {
                diags.push(ctx.diagnostic(
                    "Don't override inherited fields.",
                    parameter.name().span(),
                ));
            }
        }
        for member in declaration.members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_static() {
                continue;
            }
            for declarator in field.declarators() {
                let overrides_field = inherited_concrete_member(
                    model,
                    signatures,
                    &owner,
                    declarator.name().name(),
                    &declaration.source,
                )
                .is_some_and(|inherited| matches!(inherited.kind, MemberKind::Field { .. }));
                if overrides_field {
                    diags.push(ctx.diagnostic(
                        "Don't override inherited fields.",
                        declarator.name().span(),
                    ));
                }
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
