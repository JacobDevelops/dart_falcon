//! Retained source-order tokens, including nested string interpolations.

use falcon_analyze::AnalyzeContext;
use falcon_syntax::cst::SyntaxToken;

/// Every non-trivia token in source order, interpolations included.
pub(crate) fn significant_tokens<'a>(
    ctx: &AnalyzeContext<'a>,
) -> impl Iterator<Item = SyntaxToken<'a>> {
    ctx.tree().significant_tokens(ctx.source)
}

/// Every comment token in source order, interpolations included.
pub(crate) fn comment_tokens<'a>(
    ctx: &AnalyzeContext<'a>,
) -> impl Iterator<Item = SyntaxToken<'a>> {
    ctx.tree().comment_tokens(ctx.source)
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
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        assert!(significant_tokens(&ctx).any(|token| token.text() == "call"));
        assert!(significant_tokens(&ctx).any(|token| token.text() == "("));
        assert!(significant_tokens(&ctx).all(|token| !token.is_trivia()));
        let comments: Vec<_> = comment_tokens(&ctx).map(|token| token.text()).collect();
        assert_eq!(comments, ["// outer", "/* inner */"]);
    }
}
