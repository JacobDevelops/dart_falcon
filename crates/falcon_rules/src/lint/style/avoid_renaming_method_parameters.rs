//! Requires overridden methods to preserve inherited parameter names.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, Rule, SemanticModel, SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use super::override_members::override_member_owners;

#[derive(Default)]
pub struct AvoidRenamingMethodParameters;

impl Rule for AvoidRenamingMethodParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for (name, members) in override_member_owners(program) {
            check_members(
                model.resolve_name(name),
                members,
                signatures,
                ctx,
                &mut diags,
            );
        }
        diags
    }
}

fn positional_param_names(params: &FormalParamList) -> Vec<&Identifier> {
    params.positional().map(|param| &param.name).collect()
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
    for member in members {
        let (name, kind, params): (&str, MemberKind, Vec<&Identifier>) = match member {
            ClassMember::Method(method) if !method.is_static => (
                &method.name.name,
                MemberKind::Method,
                positional_param_names(&method.params),
            ),
            ClassMember::Operator(operator) => (
                &operator.op,
                MemberKind::Operator,
                positional_param_names(&operator.params),
            ),
            ClassMember::Setter(setter) if !setter.is_static => {
                (&setter.name.name, MemberKind::Setter, vec![&setter.param])
            }
            ClassMember::Field(_)
            | ClassMember::Constructor(_)
            | ClassMember::Method(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Error(_) => continue,
        };
        let Some(inherited) = signatures.inherited_member_facts(&owner, name) else {
            continue;
        };
        let mut flagged = HashSet::new();
        for ancestor in inherited.iter().filter(|ancestor| ancestor.kind == kind) {
            if ancestor.positional_parameter_names.len() > params.len() {
                continue;
            }
            for (index, (param, inherited_name)) in params
                .iter()
                .zip(&ancestor.positional_parameter_names)
                .enumerate()
            {
                if param.name != *inherited_name && flagged.insert(index) {
                    diags.push(ctx.diagnostic(
                        "avoid-renaming-method-parameters",
                        format!(
                            "Rename parameter '{}' to match the overridden declaration's '{}'.",
                            param.name, inherited_name
                        ),
                        param.span,
                    ));
                }
            }
        }
    }
}
