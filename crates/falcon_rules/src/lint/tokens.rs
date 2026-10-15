//! Retained source-order tokens, including nested string interpolations.

use falcon_analyze::AnalyzeContext;
use falcon_syntax::token::Token;

/// Every non-trivia token in source order, interpolations included.
pub(crate) fn significant_tokens<'a>(
    ctx: &'a AnalyzeContext<'_>,
) -> impl Iterator<Item = &'a Token> {
    ctx.syntax.significant_tokens()
}

/// Every comment token in source order, interpolations included.
pub(crate) fn comment_tokens<'a>(ctx: &'a AnalyzeContext<'_>) -> impl Iterator<Item = &'a Token> {
    ctx.syntax.comment_tokens()
}

#[cfg(test)]
mod tests {
    use super::{comment_tokens, significant_tokens};
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn cached_source_tokens_preserve_interpolation_tokens_and_comments() {
        let source = "// outer\nfinal text = '${call(1 /* inner */)}';";
        let parsed = falcon_dart_parser::parse(source);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        assert!(significant_tokens(&ctx).any(|token| token.text(source) == "call"));
        assert!(significant_tokens(&ctx).any(|token| token.text(source) == "("));
        assert!(significant_tokens(&ctx).all(|token| !token.is_trivia()));
        let comments: Vec<_> = comment_tokens(&ctx)
            .map(|token| token.text(source))
            .collect();
        assert_eq!(comments, ["// outer", "/* inner */"]);
    }
}
