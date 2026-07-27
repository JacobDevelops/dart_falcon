//! Flags instance fields that redeclare inherited concrete fields.

use falcon_analyze::{AnalyzeContext, IdentityIndex, Rule, SemanticMemberKind, SignatureIndex};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct OverriddenFields;

impl Rule for OverriddenFields {
    fn name(&self) -> &'static str {
        "overridden-fields"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(node) => check_members(
                    &node.name.name,
                    &node.members,
                    identities,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Mixin(node) => check_members(
                    &node.name.name,
                    &node.members,
                    identities,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::MixinClass(node) => check_members(
                    &node.name.name,
                    &node.members,
                    identities,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::Enum(node) => check_members(
                    &node.name.name,
                    &node.members,
                    identities,
                    signatures,
                    ctx,
                    &mut diags,
                ),
                TopLevelDecl::ExtensionType(node) => check_members(
                    &node.name.name,
                    &node.members,
                    identities,
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
    type_name: &str,
    members: &[ClassMember],
    identities: &IdentityIndex,
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = identities.resolve_declaration(ctx.file_path, &[type_name.to_string()])
    else {
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
                        fact.kind == SemanticMemberKind::Field
                            && !fact.is_abstract
                            && !fact.is_covariant
                    })
                });
            if inherited_field {
                diags.push(Diagnostic::new(
                    "overridden-fields",
                    Severity::Warning,
                    "Don't override inherited fields.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: declarator.name.span.start,
                        end: declarator.name.span.end,
                    },
                ));
            }
        }
    }
}
