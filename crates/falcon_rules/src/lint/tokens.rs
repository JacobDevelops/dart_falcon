//! Token-range queries shared by syntax-only lint rules.

use falcon_syntax::cst::{Span, SyntaxToken, SyntaxTree};

pub(crate) fn significant_tokens_in<'a>(
    tree: &'a SyntaxTree,
    source: &'a str,
    span: Span,
) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
    tree.tokens_in(source, span)
        .filter(|token| !token.is_trivia())
}

/// Trivia immediately before `offset`, nearest token first.
pub(crate) fn preceding_trivia<'a>(
    tokens: impl DoubleEndedIterator<Item = SyntaxToken<'a>>,
    offset: usize,
) -> impl Iterator<Item = SyntaxToken<'a>> {
    tokens
        .rev()
        .skip_while(move |token| token.offset() >= offset)
        .take_while(|token| token.is_trivia())
}

#[cfg(test)]
mod tests {
    use super::{preceding_trivia, significant_tokens_in};
    use falcon_syntax::cst::Span;

    #[test]
    fn source_tokens_preserve_interpolation_tokens_and_comments() {
        let source = "// outer\nfinal text = '${call(1 /* inner */)}';";
        let parsed = falcon_dart_parser::parse(source);
        let tree = &parsed.tree;
        let tokens = || significant_tokens_in(tree, source, Span::new(0, source.len()));
        assert!(tokens().any(|token| token.text() == "call"));
        assert!(tokens().any(|token| token.text() == "("));
        assert!(tokens().all(|token| !token.is_trivia()));
        let comments: Vec<_> = tree
            .comment_tokens(source)
            .map(|token| token.text())
            .collect();
        assert_eq!(comments, ["// outer", "/* inner */"]);
        let before = |offset| {
            preceding_trivia(tree.tokens(source), offset)
                .map(|token| token.text())
                .collect::<Vec<_>>()
        };
        assert_eq!(before(source.find("final").unwrap()), ["\n", "// outer"]);
        assert_eq!(before(source.find("text").unwrap()), [" "]);
    }
}
