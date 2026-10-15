//! Requires overriding methods to keep the positional parameter names of the
//! member they inherit.
//!
//! Adopted from the official `avoid_renaming_method_parameters` lint.
//! The rule only runs for libraries under the package's `lib/` directory and skips
//! static and documented methods. For a class, enum, or mixin class the
//! compared member is the concrete one inherited through the superclass chain
//! and mixin applications (abstract and `implements`-only members do not
//! count); for a mixin it is the member of its `on` types' interfaces (or
//! `Object`). Owners with private names, extension types, getters, and setters
//! are never checked. Positional parameters are compared pairwise up to the
//! shorter list.
//!
//! A `_` name on either side is skipped when the library's language version
//! enables wildcard variables (3.7+).

use falcon_analyze::{AnalyzeContext, Feature, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{Token, ast::*};

use crate::lint::inherited_members::{inherited_concrete_member, inheriting_declarations};
use crate::lint::package_layout::is_in_lib_dir;
use crate::lint::style::public_member_api_docs::documented;

#[derive(Default)]
pub struct AvoidRenamingMethodParameters;

impl Rule for AvoidRenamingMethodParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        if !is_in_lib_dir(ctx.file_path) {
            return Vec::new();
        }
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let wildcards = ctx.language_version().supports(Feature::WildcardVariables);
        let is_wildcard = |name: &str| wildcards && name == "_";
        let mut diags = Vec::new();
        for declaration in inheriting_declarations(program) {
            if declaration.name.name.starts_with('_') {
                continue;
            }
            let Some(owner) = model.resolve_name(&*declaration.name.name) else {
                continue;
            };
            for member in declaration.members {
                let (name, annotations, params, start) = match member {
                    ClassMember::Method(method) if !method.is_static => (
                        method.name.name.as_str(),
                        &method.annotations,
                        &method.params,
                        method.span.start,
                    ),
                    ClassMember::Operator(operator) => (
                        operator.lookup_name(),
                        &operator.annotations,
                        &operator.params,
                        operator.span.start,
                    ),
                    ClassMember::Method(_)
                    | ClassMember::Field(_)
                    | ClassMember::Constructor(_)
                    | ClassMember::PrimaryConstructorBody(_)
                    | ClassMember::Getter(_)
                    | ClassMember::Setter(_)
                    | ClassMember::Error(_) => continue,
                };
                let Some(parent) = inherited_concrete_member(
                    &model,
                    signatures,
                    &owner,
                    name,
                    &declaration.source,
                ) else {
                    continue;
                };
                let mut renamed = params
                    .positional()
                    .zip(&parent.positional_parameter_names)
                    .filter(|(param, parent_name)| {
                        param.name.name != **parent_name
                            && !is_wildcard(&param.name.name)
                            && !is_wildcard(parent_name)
                    })
                    .peekable();
                if renamed.peek().is_none() || is_documented(ctx.tokens(), start, annotations) {
                    continue;
                }
                diags.extend(renamed.map(|(param, parent_name)| {
                    ctx.diagnostic(
                        "avoid-renaming-method-parameters",
                        format!(
                            "Rename parameter '{}' to match the overridden declaration's '{}'.",
                            param.name.name, parent_name
                        ),
                        param.name.span,
                    )
                }));
            }
        }
        diags
    }
}

/// Whether a doc comment precedes the member or follows its annotations.
fn is_documented(tokens: &[Token], start: usize, annotations: &[Annotation]) -> bool {
    documented(tokens, start)
        || annotations.last().is_some_and(|annotation| {
            let after = tokens.partition_point(|token| token.offset < annotation.span.end);
            tokens[after..]
                .iter()
                .find(|token| !token.is_trivia())
                .is_some_and(|token| documented(tokens, token.offset))
        })
}
