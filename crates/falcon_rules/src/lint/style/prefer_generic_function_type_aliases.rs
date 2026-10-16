//! Flags old-style function typedefs written with inline parameter syntax.
//!
//! Dart's original typedef form, `typedef int F(int x);`, predates generic
//! function types and cannot express a function type in every position the
//! modern syntax can. Rewriting it as `typedef F = int Function(int);` is more
//! uniform, composes with generics, and reads consistently with other type
//! aliases. Legacy declarations retain their written formal parameter list in
//! the AST, distinguishing them from modern aliases.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::{Program, TopLevelDecl};
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct PreferGenericFunctionTypeAliases;

impl Rule for PreferGenericFunctionTypeAliases {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::TypeAlias(alias) = declaration else {
                continue;
            };
            let Some(params) = &alias.legacy_params else {
                continue;
            };
            if !ctx
                .syntax
                .tokens_in(params.span)
                .find(|token| !token.is_trivia())
                .is_some_and(|token| token.kind == TokenKind::LParen)
            {
                continue;
            }
            if let Some(keyword) = ctx
                .syntax
                .keywords_in(alias.span, TokenKind::Typedef)
                .next()
            {
                diags.push(ctx.diagnostic(
                    "prefer-generic-function-type-aliases",
                    "Use the generic function type syntax in typedefs.",
                    falcon_syntax::ast::Span {
                        start: keyword.offset(),
                        end: keyword.offset() + keyword.len(),
                    },
                ));
            }
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_alias_without_parameter_list_is_not_a_legacy_function_alias() {
        let source = "typedef F;";
        let falcon_dart_parser::ParseResult {
            program,
            syntax,
            errors,
        } = falcon_dart_parser::parse(source);
        assert!(!errors.is_empty());
        let config = falcon_config::FalconConfig::default();
        let context =
            AnalyzeContext::new(std::path::Path::new("test.dart"), source, &config, &syntax);
        assert!(
            PreferGenericFunctionTypeAliases
                .analyze(&program, &context)
                .is_empty()
        );
    }
}
