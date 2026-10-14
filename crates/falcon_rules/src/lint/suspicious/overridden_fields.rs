//! Flags instance fields that redeclare inherited concrete fields.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, SemanticModel, SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct OverriddenFields;

impl Rule for OverriddenFields {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Mixin(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::MixinClass(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Enum(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::ExtensionType(node) => check_members(
                    model.resolve_name(&node.name.name),
                    &node.members,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::ClassTypeAlias(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::Function(_)
                | TopLevelDecl::Variable(_)
                | TopLevelDecl::TypeAlias(_)
                | TopLevelDecl::Error(_) => {}
            }
        }
        diags
    }
}

fn check_members(
    owner: Option<DeclarationIdentity>,
    members: &[ClassMember],
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = owner else {
        return;
    };
    for field in members.iter().filter_map(|member| match member {
        ClassMember::Field(field) if !field.is_static => Some(field),
        ClassMember::Field(_)
        | ClassMember::Constructor(_)
        | ClassMember::Method(_)
        | ClassMember::Getter(_)
        | ClassMember::Setter(_)
        | ClassMember::Operator(_)
        | ClassMember::Error(_) => None,
    }) {
        for declarator in &field.declarators {
            let inherited_field = signatures
                .inherited_member_facts(&owner, &declarator.name.name)
                .is_some_and(|facts| {
                    facts.iter().any(|fact| {
                        matches!(
                            fact.kind,
                            MemberKind::Field {
                                covariant: false,
                                ..
                            }
                        ) && !fact.is_abstract
                    })
                });
            if inherited_field {
                diags.push(ctx.diagnostic(
                    "overridden-fields",
                    "Don't override inherited fields.",
                    declarator.name.span,
                ));
            }
        }
    }
}
