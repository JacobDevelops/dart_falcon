//! Prefer one Flutter widget class per file.
//!
//! Resolved subclasses of Flutter's `Widget` count, including indirect subclasses;
//! `State` classes and unrelated classes do not. Private widgets count by default.
//! Use `ignore-private-widgets` or `ignore-visible-for-testing` to exclude them.

use crate::lint::{declarations::has_meta_annotation, options};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, SignatureIndex, StructuralRuleVisitor,
    StructuralTraversal, Truth,
};
use falcon_analyze::{DeclarationIdentity, ResolvedType, TypeRelation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Ignore widgets whose names start with an underscore.
    pub ignore_private_widgets: bool,
    /// Ignore widgets annotated with `visibleForTesting`.
    pub ignore_visible_for_testing: bool,
}
options::impl_options!(Options);

#[derive(Default)]
pub struct PreferSingleWidgetPerFile;
impl Rule for PreferSingleWidgetPerFile {
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
            options: options::get::<Options>(ctx, Self::NAME),
            seen: false,
            diagnostics: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    options: &'a Options,
    seen: bool,
    diagnostics: Vec<Diagnostic>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, annotations) = match declaration {
            TopLevelDecl::Class(node) => (node.name(), node.annotations()),
            TopLevelDecl::MixinClass(node) => (node.name(), node.annotations()),
            _ => return StructuralTraversal::SkipChildren,
        };
        if self.options.ignore_private_widgets && name.name().starts_with('_')
            || self.options.ignore_visible_for_testing && has_meta_annotation(annotations, "visibleForTesting", &self.model)
        { return StructuralTraversal::SkipChildren; }
        if let Some(identity) = self.model.resolve_name(name.name())
            && self.signatures.relation(&ResolvedType::interface_identity(identity), TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("Widget")), &self.model) == Truth::Yes
        {
            if self.seen { self.diagnostics.push(self.ctx.diagnostic("Prefer only one widget per file.", name.span())); }
            self.seen = true;
        }
        StructuralTraversal::SkipChildren
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
