#[macro_use]
pub mod macros;

mod decl;
mod expr;
mod kind;
mod node;
mod pattern;
mod query;
mod span;
mod stmt;
mod tree;
mod types;
mod values;
pub mod visit;

pub use decl::*;
pub use expr::*;
pub use kind::NodeKind;
pub use node::{
    AstChildren, AstNode, SyntaxElement, SyntaxElementChildren, SyntaxNode, SyntaxNodeChildren,
    SyntaxToken, SyntaxTokens,
};
pub use pattern::*;
pub use query::DocBlock;
pub use stmt::*;
pub use tree::{Event, NodeId, SyntaxTree, TokenId};
pub use types::*;
pub use visit::*;

pub use span::Span;
pub use values::*;

#[derive(Clone, Copy)]
pub struct Identifier<'a>(SyntaxToken<'a>, IdentifierOrigin<'a>);

#[derive(Clone, Copy)]
enum IdentifierOrigin<'a> {
    Source,
    Missing(NodeId),
    Synthetic {
        name: &'a str,
        span: Span,
        node: NodeId,
    },
    MissingWithSpan {
        span: Span,
        node: NodeId,
    },
}

impl<'a> Identifier<'a> {
    /// A parser-confirmed name node, including zero-width missing names.
    pub fn from_node(node: SyntaxNode<'a>) -> Option<Self> {
        if node.kind() != NodeKind::Identifier {
            return None;
        }
        match node.significant_tokens().next() {
            Some(token) => Some(Self(token, IdentifierOrigin::Source)),
            None => node
                .tree()
                .token_at(node.source(), node.span().start)
                .map(|token| Self(token, IdentifierOrigin::Missing(node.id()))),
        }
    }

    pub fn synthetic(node: SyntaxNode<'a>, name: &'static str, span: Span) -> Self {
        let token = node
            .tree()
            .token_at(node.source(), span.start)
            .expect("synthetic identifier span must belong to its source");
        Self(
            token,
            IdentifierOrigin::Synthetic {
                name,
                span,
                node: node.id(),
            },
        )
    }

    pub fn with_span(self, span: Span) -> Self {
        let origin = if self.is_missing() {
            IdentifierOrigin::MissingWithSpan {
                span,
                node: self.syntax().id(),
            }
        } else {
            IdentifierOrigin::Synthetic {
                name: self.name(),
                span,
                node: self.syntax().id(),
            }
        };
        Self(self.0, origin)
    }

    /// The source name token, absent for missing, synthetic, or span-overridden names.
    pub fn token(self) -> Option<SyntaxToken<'a>> {
        matches!(self.1, IdentifierOrigin::Source).then_some(self.0)
    }

    pub fn syntax(self) -> SyntaxNode<'a> {
        let id = match self.1 {
            IdentifierOrigin::Source => return self.0.parent(),
            IdentifierOrigin::Missing(node)
            | IdentifierOrigin::Synthetic { node, .. }
            | IdentifierOrigin::MissingWithSpan { node, .. } => node,
        };
        SyntaxNode {
            tree: self.0.tree,
            source: self.0.source,
            id,
        }
    }

    pub fn is_missing(self) -> bool {
        matches!(
            self.1,
            IdentifierOrigin::Missing(_) | IdentifierOrigin::MissingWithSpan { .. }
        )
    }

    pub fn name(self) -> &'a str {
        match self.1 {
            IdentifierOrigin::Source => self.0.text(),
            IdentifierOrigin::Missing(_) | IdentifierOrigin::MissingWithSpan { .. } => "<error>",
            IdentifierOrigin::Synthetic { name, .. } => name,
        }
    }

    pub fn span(self) -> Span {
        match self.1 {
            IdentifierOrigin::Source => self.0.span(),
            IdentifierOrigin::Missing(_) => self
                .syntax()
                .recovery_span()
                .unwrap_or_else(|| self.0.span()),
            IdentifierOrigin::MissingWithSpan { span, .. } => span,
            IdentifierOrigin::Synthetic { span, .. } => span,
        }
    }
}

debug_node!(Identifier { name, span });

#[cfg(test)]
mod identifier_tests {
    use super::*;
    use crate::token::{Token, TokenKind};

    #[test]
    fn synthetic_identifiers_preserve_the_requested_name_and_recovery_span() {
        let source = "x;";
        let tokens = [
            Token::new(TokenKind::Ident, 0, 1),
            Token::new(TokenKind::Semicolon, 1, 1),
            Token::new(TokenKind::Eof, 2, 0),
        ];
        let tree = SyntaxTree::build(
            source,
            &tokens,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Start {
                    kind: NodeKind::Identifier,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Token {
                    kind: TokenKind::Ident,
                    len: 1,
                    offset: 0,
                },
                Event::Finish,
                Event::Finish,
            ],
        );
        for name in ["<field>", "<shorthand>"] {
            let identifier = Identifier::synthetic(tree.root(source), name, Span::new(1, 2));
            assert_eq!(identifier.name(), name);
            assert_eq!(identifier.span(), Span::new(1, 2));
            assert!(!identifier.is_missing());
            assert_eq!(identifier.token(), None);
            assert_eq!(
                format!("{identifier:?}"),
                format!("Identifier {{ name: {name:?}, span: Span {{ start: 1, end: 2 }} }}")
            );
            assert_eq!(identifier.syntax(), tree.root(source));
            let shifted = identifier.with_span(Span::new(2, 2));
            assert_eq!(shifted.name(), name);
            assert_eq!(shifted.span(), Span::new(2, 2));
            assert_eq!(shifted.syntax(), identifier.syntax());
            assert_eq!(shifted.token(), None);
        }
        let token = tree.tokens(source).next().unwrap();
        let identifier = Identifier::from_node(token.parent()).unwrap();
        assert_eq!(identifier.token(), Some(token));
        let source_name = identifier.with_span(Span::new(1, 2));
        assert_eq!(source_name.name(), "x");
        assert_eq!(source_name.span(), Span::new(1, 2));
        assert_eq!(source_name.syntax(), token.parent());
        assert_eq!(source_name.token(), None);
    }

    #[test]
    fn missing_identifiers_have_no_source_token() {
        let source = ";";
        let tokens = [
            Token::new(TokenKind::Semicolon, 0, 1),
            Token::new(TokenKind::Eof, 1, 0),
        ];
        let tree = SyntaxTree::build(
            source,
            &tokens,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Start {
                    kind: NodeKind::Identifier,
                    forward_parent: None,
                    offset: 0,
                },
                Event::RecoverySpan(Span::new(0, 1)),
                Event::Finish,
                Event::Finish,
            ],
        );
        let node = tree.root(source).children().next().unwrap();
        let identifier = Identifier::from_node(node).unwrap();
        assert!(identifier.is_missing());
        assert_eq!(identifier.name(), "<error>");
        assert_eq!(identifier.span(), Span::new(0, 1));
        assert_eq!(identifier.syntax(), node);
        assert_eq!(identifier.token(), None);
        let shifted = identifier.with_span(Span::new(1, 1));
        assert!(shifted.is_missing());
        assert_eq!(shifted.span(), Span::new(1, 1));
        assert_eq!(shifted.syntax(), node);
        assert_eq!(shifted.token(), None);
    }
}
