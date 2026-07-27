//! Requires overridden methods to preserve inherited parameter names.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, SemanticMemberKind, SignatureIndex};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidRenamingMethodParameters;

impl Rule for AvoidRenamingMethodParameters {
    fn name(&self) -> &'static str {
        "avoid-renaming-method-parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(_), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::Mixin(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::MixinClass(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::Enum(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
                TopLevelDecl::ExtensionType(node) => {
                    check_members(&node.name.name, &node.members, signatures, ctx, &mut diags)
                }
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

fn positional_param_names(params: &FormalParamList) -> Vec<&Identifier> {
    params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .map(|param| &param.name)
        .collect()
}

fn check_members(
    type_name: &str,
    members: &[ClassMember],
    signatures: &SignatureIndex,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(owner) = ctx.identities.and_then(|identities| {
        identities.resolve_declaration(ctx.file_path, &[type_name.to_string()])
    }) else {
        return;
    };
    for member in members {
        let (name, kind, params): (&str, SemanticMemberKind, Vec<&Identifier>) = match member {
            ClassMember::Method(method) if !method.is_static => (
                &method.name.name,
                SemanticMemberKind::Method,
                positional_param_names(&method.params),
            ),
            ClassMember::Operator(operator) => (
                &operator.op,
                SemanticMemberKind::Operator,
                positional_param_names(&operator.params),
            ),
            ClassMember::Setter(setter) if !setter.is_static => (
                &setter.name.name,
                SemanticMemberKind::Setter,
                vec![&setter.param],
            ),
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
                    diags.push(Diagnostic::new(
                        "avoid-renaming-method-parameters",
                        Severity::Warning,
                        format!(
                            "Rename parameter '{}' to match the overridden declaration's '{}'.",
                            param.name, inherited_name
                        ),
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: param.span.start,
                            end: param.span.end,
                        },
                    ));
                }
            }
        }
    }
}
