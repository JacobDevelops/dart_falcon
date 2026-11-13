//! Flags old-style function typedefs written with inline parameter syntax.
//!
//! Dart's original typedef form, `typedef int F(int x);`, predates generic
//! function types and cannot express a function type in every position the
//! modern syntax can. Rewriting it as `typedef F = int Function(int);` is more
//! uniform, composes with generics, and reads consistently with other type
//! aliases. Legacy declarations retain their written formal parameter list in
//! the syntax tree, distinguishing them from modern aliases.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Program, TopLevelDecl};
use falcon_syntax::token::TokenKind;

use crate::lint::tokens::significant_tokens_in;

#[derive(Default)]
pub struct PreferGenericFunctionTypeAliases;

impl Rule for PreferGenericFunctionTypeAliases {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let TopLevelDecl::TypeAlias(alias) = declaration else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(params) = alias.legacy_params() else {
            return StructuralTraversal::SkipChildren;
        };
        if !significant_tokens_in(ctx.tree(), ctx.source(), params.span())
            .next()
            .is_some_and(|token| token.kind() == TokenKind::LParen)
        {
            return StructuralTraversal::SkipChildren;
        }
        self.diags.push(ctx.diagnostic(
            "Use the generic function type syntax in typedefs.",
            alias.name().span(),
        ));
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn reports_the_alias_name() {
        let source = "typedef int Compare(int left, int right);";
        let diagnostics = crate::lint::test_support::diagnostics(
            "prefer-generic-function-type-aliases",
            &PreferGenericFunctionTypeAliases,
            std::path::Path::new("test.dart"),
            source,
            false,
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "Compare"
        );
    }

    #[test]
    fn incomplete_alias_without_parameter_list_is_not_a_legacy_function_alias() {
        let source = "typedef F;";
        let (snapshot, errors) = falcon_analyze::ParsedFile::parse_for_package(source, None);
        let parsed = std::sync::Arc::new(snapshot);
        let program = parsed.program();
        assert!(!errors.is_empty());
        let config = falcon_config::FalconConfig::default();
        let context = AnalyzeContext::new(std::path::Path::new("test.dart"), &config, &parsed);
        assert!(
            PreferGenericFunctionTypeAliases
                .analyze(PreferGenericFunctionTypeAliases::NAME, program, &context)
                .is_empty()
        );
    }
}
