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

use falcon_analyze::{AnalyzeContext, MemberKind, Rule, RulePreparation, SemanticModel};
use falcon_syntax::ast::*;

use crate::lint::inherited_members::{inherited_concrete_member, inheriting_declarations};

#[derive(Default)]
pub struct OverriddenFields;

impl Rule for OverriddenFields {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for declaration in inheriting_declarations(program) {
            let Some(owner) = model.resolve_name(&*declaration.name.name) else {
                continue;
            };
            for parameter in (EffectiveMembers {
                declared: declaration.members,
                primary: declaration.primary_constructor,
                representation: false,
            }
            .declaring_params())
            {
                if inherited_concrete_member(
                    &model,
                    signatures,
                    &owner,
                    &parameter.name.name,
                    &declaration.source,
                )
                .is_some_and(|inherited| matches!(inherited.kind, MemberKind::Field { .. }))
                {
                    diags.push(ctx.diagnostic(
                        "overridden-fields",
                        "Don't override inherited fields.",
                        parameter.name.span,
                    ));
                }
            }
            for member in declaration.members {
                let ClassMember::Field(field) = member else {
                    continue;
                };
                if field.is_static {
                    continue;
                }
                for declarator in &field.declarators {
                    let overrides_field = inherited_concrete_member(
                        &model,
                        signatures,
                        &owner,
                        &declarator.name.name,
                        &declaration.source,
                    )
                    .is_some_and(|inherited| matches!(inherited.kind, MemberKind::Field { .. }));
                    if overrides_field {
                        diags.push(ctx.diagnostic(
                            "overridden-fields",
                            "Don't override inherited fields.",
                            declarator.name.span,
                        ));
                    }
                }
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}
