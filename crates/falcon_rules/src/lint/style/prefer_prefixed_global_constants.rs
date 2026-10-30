//! Require configured prefixes on top-level `const` variables.
//!
//! `allowed-prefixes` defaults to `["k"]`; an empty list disables the rule.
//! Leading underscores are ignored when matching. Set `ignore-private` (default
//! false) to exempt private constants. Final and mutable globals, fields and
//! locals do not count.

use crate::lint::options;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Accepted prefixes after leading underscores; an empty list disables the rule.
    pub allowed_prefixes: Vec<String>,
    /// Ignore private top-level constants instead of checking their prefixes.
    pub ignore_private: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            allowed_prefixes: vec!["k".into()],
            ignore_private: false,
        }
    }
}
options::impl_options!(Options);

#[derive(Default)]
pub struct PreferPrefixedGlobalConstants;
impl Rule for PreferPrefixedGlobalConstants {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let configured = options::get::<Options>(ctx, Self::NAME);
        if configured.allowed_prefixes.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let prefixes = configured
            .allowed_prefixes
            .iter()
            .map(|prefix| format!("`{prefix}`"))
            .collect::<Vec<_>>()
            .join(", ");
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            options: configured,
            message: format!("Prefix global constants with one of: {prefixes}."),
            diagnostics: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    options: &'a Options,
    message: String,
    diagnostics: Vec<Diagnostic>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(variable) = declaration
            && variable.keyword() == VarKeyword::Const
        {
            for declarator in variable.declarators() {
                let name = declarator.name();
                if self.options.ignore_private && name.name().starts_with('_') {
                    continue;
                }
                let public_name = name.name().trim_start_matches('_');
                let prefixed = self.options.allowed_prefixes.iter().any(|prefix| public_name.starts_with(prefix));
                if !prefixed {
                    self.diagnostics.push(self.ctx.diagnostic(self.message.clone(), name.span()));
                }
            }
        }
        StructuralTraversal::SkipChildren
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
