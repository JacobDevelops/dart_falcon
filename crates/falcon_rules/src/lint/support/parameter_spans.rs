use crate::lint::declarations::documented_node_span;
use crate::lint::tokens::significant_tokens_in;
use falcon_analyze::AnalyzeContext;
use falcon_syntax::{
    TokenKind,
    cst::{SetterDecl, Span},
};

/// Includes the setter's full parameter range rather than just its name.
pub(crate) fn setter_parameter_span(node: SetterDecl<'_>, ctx: &AnalyzeContext) -> Span {
    let start = significant_tokens_in(
        ctx.tree(),
        ctx.source(),
        Span::new(node.name().span().end, node.param().span().end),
    )
    .skip_while(|token| token.kind() != TokenKind::LParen)
    .nth(1)
    .map_or(node.param().span().start, |token| token.offset());
    documented_node_span(
        Span::new(start, node.param().span().end),
        node.param_annotations(),
        ctx,
    )
}
