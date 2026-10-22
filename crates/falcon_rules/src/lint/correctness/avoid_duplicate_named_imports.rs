//! Flags imports of the same library that differ only in their name prefix.
//! Conditional URIs, deferred loading, combinators, and metadata must match.
//! Consolidate uses under one import before removing the other directive.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::{TokenKind, cst};

#[derive(Default)]
pub struct AvoidDuplicateNamedImports;

impl Rule for AvoidDuplicateNamedImports {
    fn prepare<'a>(
        &self,
        program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for (index, directive) in program.imports().enumerate() {
            if program.imports().take(index).any(|earlier| {
                (earlier.as_name().is_some() || directive.as_name().is_some())
                    && earlier.uri().value() == directive.uri().value()
                    && without_prefix(earlier).eq(without_prefix(directive))
            }) {
                diags.push(ctx.diagnostic("This library is already imported with the same visibility — consolidate uses under one import and remove the duplicate.", directive.span()));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn without_prefix(directive: cst::ImportDirective<'_>) -> impl Iterator<Item = (TokenKind, &str)> {
    let prefix = directive.as_name().map(|name| name.span());
    let uri = directive.uri().span();
    directive
        .syntax()
        .significant_tokens()
        .filter(move |token| {
            !uri.contains(token.span())
                && !prefix.is_some_and(|span| {
                    token.span() == span
                        || (token.kind() == TokenKind::As
                            && token
                                .next_significant()
                                .is_some_and(|next| next.span() == span))
                })
        })
        .map(|token| (token.kind(), token.text()))
}
