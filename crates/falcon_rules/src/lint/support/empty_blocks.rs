use falcon_syntax::cst::*;

/// Closing brace of an empty block without an explanatory comment.
pub(crate) fn empty_block_end(block: Block<'_>) -> Option<Span> {
    if !block.stmts().is_empty() {
        return None;
    }
    let mut tokens = block.syntax().tokens();
    let open = tokens.next()?;
    let close = tokens.next_back()?;
    if open.kind() != falcon_syntax::TokenKind::LBrace
        || close.kind() != falcon_syntax::TokenKind::RBrace
    {
        return None;
    }
    if tokens.any(|token| token.kind().is_comment()) {
        return None;
    }
    Some(Span::new(close.offset(), close.offset() + close.len()))
}
