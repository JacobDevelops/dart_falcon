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

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, Feature, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::inherited_members::{
    InheritanceSource, inherited_concrete_member, inheriting_declaration,
};
use crate::lint::package_layout::is_in_lib_dir;
use crate::lint::style::public_member_api_docs::documented;

#[derive(Default)]
pub struct AvoidRenamingMethodParameters;

impl Rule for AvoidRenamingMethodParameters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if !is_in_lib_dir(ctx.file_path) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            tokens: ctx.tokens().collect(),
            model: SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types),
            signatures,
            owner: None,
            wildcards: ctx.language_version().supports(Feature::WildcardVariables),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    tokens: Vec<SyntaxToken<'a>>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    owner: Option<(DeclarationIdentity, InheritanceSource<'a>)>,
    wildcards: bool,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = inheriting_declaration(node)
            .filter(|declaration| !declaration.name.name().starts_with('_'))
            .and_then(|declaration| {
                self.model.resolve_name(declaration.name.name())
                    .map(|owner| (owner, declaration.source))
            });
        if self.owner.is_some() {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let Some((owner, source)) = &self.owner else {
            return StructuralTraversal::SkipChildren;
        };
        let (name, annotations, params, start) = match member {
            ClassMember::Method(method) if !method.is_static() => (
                method.name().name(), method.annotations(), method.params(), method.span().start,
            ),
            ClassMember::Operator(operator) => (
                operator.lookup_name(), operator.annotations(), operator.params(), operator.span().start,
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(parent) = inherited_concrete_member(
            &self.model, self.signatures, owner, name, source,
        ) else {
            return StructuralTraversal::SkipChildren;
        };
        let is_wildcard = |name: &str| self.wildcards && name == "_";
        let mut renamed = params.positional()
            .zip(&parent.positional_parameter_names)
            .filter(|(param, parent_name)| {
                param.name().name() != **parent_name
                    && !is_wildcard(param.name().name())
                    && !is_wildcard(parent_name)
            })
            .peekable();
        if renamed.peek().is_some() && !is_documented(&self.tokens, start, annotations) {
            self.diags.extend(renamed.map(|(param, parent_name)| {
                self.ctx.diagnostic(
                    "avoid-renaming-method-parameters",
                    format!(
                        "Rename parameter '{}' to match the overridden declaration's '{}'.",
                        param.name().name(), parent_name
                    ),
                    param.name().span(),
                )
            }));
        }
        StructuralTraversal::SkipChildren
    }
}
}

/// Whether a doc comment precedes the member or follows its annotations.
fn is_documented(
    tokens: &[SyntaxToken<'_>],
    start: usize,
    annotations: AstChildren<'_, Annotation<'_>>,
) -> bool {
    documented(tokens, start)
        || annotations.last().is_some_and(|annotation| {
            let after = tokens.partition_point(|token| token.offset() < annotation.span().end);
            tokens[after..]
                .iter()
                .find(|token| !token.is_trivia())
                .is_some_and(|token| documented(tokens, token.offset()))
        })
}
