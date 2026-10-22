use falcon_syntax::{TokenKind, cst::SyntaxToken};

pub(crate) fn documented(tokens: &[SyntaxToken<'_>], start: usize) -> bool {
    let end = tokens.partition_point(|token| token.offset() < start);
    crate::lint::tokens::preceding_trivia(tokens[..end].iter().copied(), start)
        // The lexer leaves `/**/` as ordinary trivia; analyzer still attaches
        // that empty block as a declaration's documentation comment.
        .any(|t| {
            t.kind() == TokenKind::DocComment
                || (t.kind() == TokenKind::BlockComment && t.len() == 4)
        })
}
