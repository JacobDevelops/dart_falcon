//! Require Equatable semantics for configured class/file name patterns.
//! Configure `class-name-pattern` or `file-name-pattern`; without either option,
//! this rule reports no diagnostics. Already equatable subtypes (including
//! `EquatableMixin` users) and unresolved inheritance are conservatively skipped.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use crate::lint::options;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Regex selecting class names that must use Equatable semantics.
    class_name_pattern: Option<options::Regex>,
    /// Regex selecting file names whose classes must use Equatable semantics.
    file_name_pattern: Option<options::Regex>,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct ExtendEquatable;

impl Rule for ExtendEquatable {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = options::get::<Options>(ctx, Self::NAME);
        let class_pattern = options.class_name_pattern.as_ref();
        let file_matches = options
            .file_name_pattern
            .as_ref()
            .is_some_and(|pattern| {
                ctx.file_path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| pattern.is_match(name))
            });
        if class_pattern.is_none() && !file_matches {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            class_pattern,
            file_matches,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    class_pattern: Option<&'a options::Regex>,
    file_matches: bool,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(class) => class.name(),
            _ => return StructuralTraversal::SkipChildren,
        };
        if !self.file_matches && !self.class_pattern.as_ref().is_some_and(|pattern| pattern.is_match(name.name())) {
            return StructuralTraversal::SkipChildren;
        }
        let (Some(identities), Some(signatures)) = (self.ctx.identities(), self.ctx.signatures()) else {
            return StructuralTraversal::SkipChildren;
        };
        let model = SemanticModel::new(self.ctx.file_path(), self.ctx.parsed(), identities, self.ctx.types());
        let Some(identity) = model.resolve_name(name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        if signatures.supertype_matches(&identity, &|identity| {
            identities.is_package_declaration(identity, "equatable", "Equatable")
                || identities.is_package_declaration(identity, "equatable", "EquatableMixin")
        }) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic(
                "This class matches the configured pattern and should extend `Equatable` or mix in `EquatableMixin`.",
                name.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
