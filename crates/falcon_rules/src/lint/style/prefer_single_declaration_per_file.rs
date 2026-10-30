//! Prefer one top-level type declaration per file.
//!
//! Classes, class aliases, mixin classes, mixins, enums, named extensions and
//! extension types count toward the limit; functions, variables and typedefs do not. Private
//! declarations are ignored by default. `ignore-enums` and `ignore-visible-for-
//! testing` optionally exclude additional declarations.

use crate::lint::{declarations::has_meta_annotation, options};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Ignore declarations whose names start with an underscore.
    pub ignore_private: bool,
    /// Ignore enum declarations.
    pub ignore_enums: bool,
    /// Ignore declarations annotated with `visibleForTesting` from `package:meta`.
    pub ignore_visible_for_testing: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ignore_private: true,
            ignore_enums: false,
            ignore_visible_for_testing: false,
        }
    }
}
options::impl_options!(Options);

#[derive(Default)]
pub struct PreferSingleDeclarationPerFile;

impl Rule for PreferSingleDeclarationPerFile {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            options: options::get::<Options>(ctx, Self::NAME),
            model: ctx.identities().map(|identities| {
                SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
            }),
            seen: false,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    options: &'a Options,
    model: Option<SemanticModel<'a>>,
    seen: bool,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, annotations) = match declaration {
            TopLevelDecl::Class(node) => (Some(node.name()), node.annotations()),
            TopLevelDecl::ClassTypeAlias(node) => (Some(node.name()), node.annotations()),
            TopLevelDecl::MixinClass(node) => (Some(node.name()), node.annotations()),
            TopLevelDecl::Mixin(node) => (Some(node.name()), node.annotations()),
            TopLevelDecl::Enum(node) if !self.options.ignore_enums => (Some(node.name()), node.annotations()),
            TopLevelDecl::Extension(node) => (node.name(), node.annotations()),
            TopLevelDecl::ExtensionType(node) => (Some(node.name()), node.annotations()),
            _ => return StructuralTraversal::SkipChildren,
        };
        if let Some(name) = name
            && !(self.options.ignore_private && name.name().starts_with('_'))
            && !(self.options.ignore_visible_for_testing && self.model.as_ref().is_some_and(|model| has_meta_annotation(annotations, "visibleForTesting", model)))
        {
            if self.seen {
                self.diagnostics.push(self.ctx.diagnostic("Prefer only one top-level type declaration per file.", name.span()));
            }
            self.seen = true;
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
