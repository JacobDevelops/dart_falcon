//! Flags old-style function typedefs written with inline parameter syntax.
//!
//! Dart's original typedef form, `typedef int F(int x);`, predates generic
//! function types and cannot express a function type in every position the
//! modern syntax can. Rewriting it as `typedef F = int Function(int);` is more
//! uniform, composes with generics, and reads consistently with other type
//! aliases. Legacy declarations retain their written formal parameter list in
//! the AST, distinguishing them from modern aliases.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Program, TopLevelDecl};
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct PreferGenericFunctionTypeAliases;

impl Rule for PreferGenericFunctionTypeAliases {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
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
        if !ctx
            .tree()
            .tokens_in(ctx.source, params.span())
            .find(|token| !token.is_trivia())
            .is_some_and(|token| token.kind() == TokenKind::LParen)
        {
            return StructuralTraversal::SkipChildren;
        }
        if let Some(keyword) = ctx
            .tree()
            .keywords_in(ctx.source, alias.span(), TokenKind::Typedef)
            .next()
        {
            self.diags.push(ctx.diagnostic(
                "prefer-generic-function-type-aliases",
                "Use the generic function type syntax in typedefs.",
                falcon_syntax::cst::Span {
                    start: keyword.offset(),
                    end: keyword.offset() + keyword.len(),
                },
            ));
        }
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
    fn incomplete_alias_without_parameter_list_is_not_a_legacy_function_alias() {
        let source = "typedef F;";
        let falcon_dart_parser::ParseResult { tree, errors, .. } =
            falcon_dart_parser::parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(!errors.is_empty());
        let config = falcon_config::FalconConfig::default();
        let context = AnalyzeContext::new(std::path::Path::new("test.dart"), &config, &parsed);
        assert!(
            PreferGenericFunctionTypeAliases
                .analyze(program, &context)
                .is_empty()
        );
    }
}
