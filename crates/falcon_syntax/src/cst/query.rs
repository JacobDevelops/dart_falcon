use std::ops::Range;

use super::kind::NodeKind;
use super::node::{SyntaxNode, SyntaxToken, SyntaxTokens};
use super::tree::{NodeId, SyntaxTree, TokenId};
use crate::ast::Span;
use crate::token::TokenKind;

fn is_interpolation(node: SyntaxNode<'_>) -> bool {
    node.kind() == NodeKind::StringInterpolation
        || (node.kind() == NodeKind::Error
            && node.significant_tokens().next().is_some_and(|token| {
                matches!(
                    token.kind(),
                    TokenKind::InterpolationStart | TokenKind::InterpolationDollar
                )
            }))
}

#[derive(Clone)]
struct TopLevelTokens<'a> {
    tree: &'a SyntaxTree,
    source: &'a str,
    tokens: Range<usize>,
    nodes: Range<usize>,
    front: Option<Range<usize>>,
    back: Option<Range<usize>>,
}

impl TopLevelTokens<'_> {
    fn boundary(&mut self, reverse: bool) -> Option<Range<usize>> {
        while let Some(index) = if reverse {
            self.nodes.next_back()
        } else {
            self.nodes.next()
        } {
            let node = self.tree.node(self.source, NodeId(index as u32))?;
            if !is_interpolation(node) {
                continue;
            }
            let node = if reverse {
                node.ancestors()
                    .filter(|node| is_interpolation(*node))
                    .last()
                    .unwrap()
            } else {
                node
            };
            let data = &self.tree.nodes[node.id().0 as usize];
            if reverse {
                self.nodes.end = node.id().0 as usize;
            } else {
                self.nodes.start = data.subtree_end;
            }
            return Some(data.tokens.clone());
        }
        None
    }
}

impl<'a> Iterator for TopLevelTokens<'a> {
    type Item = SyntaxToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.tokens.is_empty() {
            if self.front.is_none() {
                self.front = self.boundary(false).or_else(|| self.back.take());
            }
            if let Some(boundary) = &self.front
                && boundary.start <= self.tokens.start
            {
                self.tokens.start = self.tokens.start.max(boundary.end).min(self.tokens.end);
                self.front = None;
                continue;
            }
            return self
                .tree
                .token(self.source, TokenId(self.tokens.next()? as u32));
        }
        None
    }
}

impl DoubleEndedIterator for TopLevelTokens<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while !self.tokens.is_empty() {
            if self.back.is_none() {
                self.back = self.boundary(true).or_else(|| self.front.take());
            }
            if let Some(boundary) = &self.back
                && boundary.end >= self.tokens.end
            {
                self.tokens.end = self.tokens.end.min(boundary.start).max(self.tokens.start);
                self.back = None;
                continue;
            }
            return self
                .tree
                .token(self.source, TokenId(self.tokens.next_back()? as u32));
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct DocBlock<'a> {
    pub start: usize,
    pub next_token: usize,
    tokens: SyntaxTokens<'a>,
}

impl<'a> DocBlock<'a> {
    pub fn tokens(&self) -> impl Iterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens
            .clone()
            .filter(|token| token.kind() == TokenKind::DocComment)
    }
}

impl SyntaxTree {
    pub fn tokens<'a>(&'a self, source: &'a str) -> SyntaxTokens<'a> {
        debug_assert_eq!(self.source_len, source.len());
        SyntaxTokens::new(self, source, 0..self.tokens.len())
    }

    /// Root-stream tokens in source order, retaining trivia and literal pieces
    /// but excluding interpolation delimiters and bodies.
    pub fn top_level_tokens<'a>(
        &'a self,
        source: &'a str,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        debug_assert_eq!(self.source_len, source.len());
        TopLevelTokens {
            tree: self,
            source,
            tokens: 0..self.tokens.len(),
            nodes: 0..self.nodes.len(),
            front: None,
            back: None,
        }
    }

    pub fn token<'a>(&'a self, source: &'a str, id: TokenId) -> Option<SyntaxToken<'a>> {
        debug_assert_eq!(self.source_len, source.len());
        self.tokens.get(id.index())?;
        Some(SyntaxToken {
            tree: self,
            source,
            id,
        })
    }

    /// Returns the token containing the byte offset, or EOF at the source end.
    pub fn token_at<'a>(&'a self, source: &'a str, offset: usize) -> Option<SyntaxToken<'a>> {
        let index = self
            .tokens
            .partition_point(|token| token.offset() <= offset)
            .checked_sub(1)?;
        let token = self.token(source, TokenId(index as u32))?;
        (offset < token.span().end || (token.kind() == TokenKind::Eof && offset == self.source_len))
            .then_some(token)
    }

    /// Returns the root-stream token at an offset, or `None` inside interpolation.
    pub fn top_level_token_at<'a>(
        &'a self,
        source: &'a str,
        offset: usize,
    ) -> Option<SyntaxToken<'a>> {
        self.token_at(source, offset)
            .filter(|token| !token.parent().ancestors().any(is_interpolation))
    }

    /// Tokens fully contained in the half-open source span, excluding EOF.
    pub fn tokens_in<'a>(&'a self, source: &'a str, span: Span) -> SyntaxTokens<'a> {
        debug_assert_eq!(self.source_len, source.len());
        let start = self
            .tokens
            .partition_point(|token| token.offset() < span.start);
        let count = self.tokens[start..].partition_point(|token| {
            token.offset() < span.end
                && !token.is_empty()
                && token.offset() + token.len() <= span.end
        });
        SyntaxTokens::new(self, source, start..start + count)
    }

    pub fn significant_tokens<'a>(
        &'a self,
        source: &'a str,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens(source)
            .filter(|token| !token.is_trivia() && token.kind() != TokenKind::Eof)
    }
    pub fn comment_tokens<'a>(
        &'a self,
        source: &'a str,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens(source)
            .filter(|token| token.kind().is_comment())
    }
    pub fn identifier_tokens<'a>(
        &'a self,
        source: &'a str,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens(source).filter(|token| {
            token.kind() == TokenKind::Ident
                || (token.kind().is_identifier_like()
                    && token.parent().ancestors().any(|node| {
                        node.kind() == NodeKind::StringInterpolation
                            || (node.kind() == NodeKind::Error
                                && node.significant_tokens().next().is_some_and(|first| {
                                    matches!(
                                        first.kind(),
                                        TokenKind::InterpolationStart
                                            | TokenKind::InterpolationDollar
                                    )
                                }))
                    }))
        })
    }
    pub fn keywords_in<'a>(
        &'a self,
        source: &'a str,
        span: Span,
        kind: TokenKind,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens_in(source, span)
            .filter(move |token| token.kind() == kind)
    }
    pub fn comments_in<'a>(
        &'a self,
        source: &'a str,
        span: Span,
    ) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens_in(source, span)
            .filter(|token| token.kind().is_comment())
    }

    /// Canonical documentation groups attached to the following code token or EOF.
    pub fn doc_blocks<'a>(&'a self, source: &'a str) -> impl Iterator<Item = DocBlock<'a>> + Clone {
        let mut tokens = self.top_level_tokens(source);
        std::iter::from_fn(move || {
            let mut start = None;
            for token in tokens.by_ref() {
                if token.kind() == TokenKind::DocComment {
                    if start.is_none() || token.text().starts_with("/**") {
                        start = Some(token.id());
                    }
                } else if !token.is_trivia()
                    && let Some(start) = start.take()
                {
                    return Some(DocBlock {
                        start: self.tokens[start.index()].offset(),
                        next_token: token.offset(),
                        tokens: SyntaxTokens::new(self, source, start.index()..token.id().index()),
                    });
                }
            }
            None
        })
    }

    pub fn documentation_start(&self, gap: Span, source: &str) -> Option<usize> {
        let mut start = None;
        let mut previous_block = false;
        for token in self
            .tokens_in(source, gap)
            .filter(|token| token.kind() == TokenKind::DocComment)
        {
            let is_block = token.text().starts_with("/**");
            if is_block || previous_block || start.is_none() {
                start = Some(token.offset());
            }
            previous_block = is_block;
        }
        start
    }

    pub fn closing_delimiter<'a>(
        &'a self,
        source: &'a str,
        open: TokenId,
    ) -> Option<SyntaxToken<'a>> {
        matches!(
            self.token(source, open)?.kind(),
            TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::QmarkLBracket
                | TokenKind::LBrace
                | TokenKind::InterpolationStart
        )
        .then(|| self.paired_delimiter(source, open))
        .flatten()
    }
    pub fn opening_delimiter<'a>(
        &'a self,
        source: &'a str,
        close: TokenId,
    ) -> Option<SyntaxToken<'a>> {
        matches!(
            self.token(source, close)?.kind(),
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
        )
        .then(|| self.paired_delimiter(source, close))
        .flatten()
    }
    fn paired_delimiter<'a>(&'a self, source: &'a str, id: TokenId) -> Option<SyntaxToken<'a>> {
        let pairs = self.delimiter_pairs.get_or_init(|| {
            let mut pairs = vec![u32::MAX; self.tokens.len()];
            let mut stacks = [u32::MAX; 3];
            let mut interpolations = self
                .root(source)
                .descendants()
                .filter(|node| is_interpolation(*node))
                .map(|node| self.nodes[node.id().0 as usize].tokens.clone())
                .filter(|range| !range.is_empty())
                .peekable();
            let mut outer_stacks = Vec::new();
            let clear_stacks = |stacks: [u32; 3], pairs: &mut [u32]| {
                for mut open in stacks {
                    while open != u32::MAX {
                        let previous = pairs[open as usize];
                        pairs[open as usize] = u32::MAX;
                        open = previous;
                    }
                }
            };
            for (index, token) in self.tokens.iter().enumerate() {
                while outer_stacks.last().is_some_and(|(end, _)| *end <= index) {
                    clear_stacks(stacks, &mut pairs);
                    stacks = outer_stacks.pop().unwrap().1;
                }
                while interpolations
                    .peek()
                    .is_some_and(|range| range.start == index)
                {
                    let range = interpolations.next().unwrap();
                    outer_stacks.push((range.end, stacks));
                    stacks = [u32::MAX; 3];
                }
                let (stack, opening) = match token.kind {
                    TokenKind::LParen => (0, true),
                    TokenKind::LBracket | TokenKind::QmarkLBracket => (1, true),
                    TokenKind::LBrace | TokenKind::InterpolationStart => (2, true),
                    TokenKind::RParen => (0, false),
                    TokenKind::RBracket => (1, false),
                    TokenKind::RBrace => (2, false),
                    _ => continue,
                };
                if opening {
                    pairs[index] = stacks[stack];
                    stacks[stack] = index as u32;
                } else if stacks[stack] != u32::MAX {
                    let open = stacks[stack] as usize;
                    stacks[stack] = pairs[open];
                    pairs[open] = index as u32;
                    pairs[index] = open as u32;
                }
            }
            clear_stacks(stacks, &mut pairs);
            for (_, stacks) in outer_stacks {
                clear_stacks(stacks, &mut pairs);
            }
            pairs.into_boxed_slice()
        });
        let paired = *pairs.get(id.index())?;
        (paired != u32::MAX)
            .then(|| self.token(source, TokenId(paired)))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tree::Event;
    use super::*;
    use crate::token::{SyntaxStore, Token, TokenId as StoreTokenId};

    fn fixture() -> (&'static str, Vec<Token>) {
        let source = "/// a\n/** b */\n/// c\nf([)] // tail\n";
        let pieces = [
            (TokenKind::DocComment, "/// a"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/** b */"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/// c"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Ident, "f"),
            (TokenKind::LParen, "("),
            (TokenKind::LBracket, "["),
            (TokenKind::RParen, ")"),
            (TokenKind::RBracket, "]"),
            (TokenKind::Whitespace, " "),
            (TokenKind::LineComment, "// tail"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Eof, ""),
        ];
        let mut offset = 0;
        let tokens = pieces
            .into_iter()
            .map(|(kind, text)| {
                let token = Token::new(kind, offset, text.len());
                offset += text.len();
                token
            })
            .collect();
        (source, tokens)
    }

    #[test]
    fn flat_queries_match_syntax_store() {
        let (source, tokens) = fixture();
        let store = SyntaxStore::new(tokens.clone());
        let tree = SyntaxTree::build(
            source,
            &tokens,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Finish,
            ],
        );
        let spans = |tokens: Vec<SyntaxToken<'_>>| {
            tokens
                .into_iter()
                .map(|token| (token.kind(), token.span()))
                .collect::<Vec<_>>()
        };
        let old = |tokens: Vec<&Token>| {
            tokens
                .into_iter()
                .map(|token| {
                    (
                        token.kind,
                        Span::new(token.offset(), token.offset() + token.len()),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            spans(tree.tokens(source).collect()),
            old(store.tokens().iter().collect())
        );
        assert_eq!(
            spans(tree.significant_tokens(source).collect()),
            old(store.significant_tokens().collect())
        );
        assert_eq!(
            spans(tree.comment_tokens(source).collect()),
            old(store.comment_tokens().collect())
        );
        assert_eq!(
            spans(tree.identifier_tokens(source).collect()),
            old(store.identifier_tokens().collect())
        );
        for start in 0..=source.len() {
            for end in start..=source.len() {
                let span = Span::new(start, end);
                assert_eq!(
                    spans(tree.tokens_in(source, span).collect()),
                    old(store.tokens_in(span).collect())
                );
                assert_eq!(
                    spans(tree.comments_in(source, span).collect()),
                    old(store.comments_in(span).collect())
                );
                assert_eq!(
                    spans(tree.keywords_in(source, span, TokenKind::Ident).collect()),
                    old(store.keywords_in(span, TokenKind::Ident).collect())
                );
                assert_eq!(
                    tree.documentation_start(span, source),
                    store.documentation_start(span, source)
                );
            }
        }
        for index in 0..tokens.len() {
            assert_eq!(
                tree.closing_delimiter(source, TokenId(index as u32))
                    .map(|token| token.id().0),
                store
                    .closing_delimiter(StoreTokenId(index as u32))
                    .map(|id| id.0)
            );
            assert_eq!(
                tree.opening_delimiter(source, TokenId(index as u32))
                    .map(|token| token.id().0),
                store
                    .opening_delimiter(StoreTokenId(index as u32))
                    .map(|id| id.0)
            );
        }
        let blocks: Vec<_> = tree
            .doc_blocks(source)
            .map(|block| {
                (
                    block.start,
                    block.next_token,
                    block.tokens().map(|token| token.text()).collect::<Vec<_>>(),
                )
            })
            .collect();
        let expected: Vec<_> = store
            .doc_blocks(source)
            .map(|block| {
                (
                    block.start,
                    block.next_token,
                    block
                        .tokens()
                        .map(|token| token.text(source))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(blocks, expected);
        for token in tree.tokens(source) {
            assert_eq!(tree.token_at(source, token.offset()), Some(token));
        }
        assert!(tree.token_at(source, source.len() + 1).is_none());
    }

    fn interpolation_fixture() -> (String, SyntaxTree) {
        let pieces = [
            (TokenKind::DocComment, "/// outer"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Ident, "f"),
            (TokenKind::LParen, "("),
            (TokenKind::StringPart, "\"a"),
            (TokenKind::InterpolationStart, "${"),
            (TokenKind::DocComment, "/** hidden */"),
            (TokenKind::Whitespace, " "),
            (TokenKind::Ident, "g"),
            (TokenKind::LParen, "("),
            (TokenKind::StringPart, "'"),
            (TokenKind::InterpolationStart, "${"),
            (TokenKind::Ident, "h"),
            (TokenKind::LParen, "("),
            (TokenKind::RBrace, "}"),
            (TokenKind::StringPart, "'"),
            (TokenKind::RParen, ")"),
            (TokenKind::LBracket, "["),
            (TokenKind::RBrace, "}"),
            (TokenKind::StringPart, "b"),
            (TokenKind::InterpolationStart, "${"),
            (TokenKind::RParen, ")"),
            (TokenKind::RBracket, "]"),
            (TokenKind::RBrace, "}"),
            (TokenKind::StringPart, "c"),
            (TokenKind::InterpolationDollar, "$"),
            (TokenKind::Ident, "name"),
            (TokenKind::StringPart, "\""),
            (TokenKind::RParen, ")"),
            (TokenKind::Whitespace, " "),
            (TokenKind::LineComment, "// tail"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/// after"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Ident, "z"),
        ];
        let source: String = pieces.iter().map(|(_, text)| *text).collect();
        let mut events = vec![Event::Start {
            kind: NodeKind::Program,
            forward_parent: None,
            offset: 0,
        }];
        let mut offset = 0;
        for (index, (kind, text)) in pieces.into_iter().enumerate() {
            let interpolation = match index {
                5 | 25 => Some(NodeKind::StringInterpolation),
                11 | 20 => Some(NodeKind::Error),
                _ => None,
            };
            if let Some(kind) = interpolation {
                events.push(Event::Start {
                    kind,
                    forward_parent: None,
                    offset,
                });
            }
            events.push(Event::Token {
                kind,
                offset,
                len: text.len(),
            });
            offset += text.len();
            if matches!(index, 14 | 18 | 23 | 26) {
                events.push(Event::Finish);
            }
        }
        events.push(Event::Finish);
        let tree = SyntaxTree::build(&source, &[], events);
        (source, tree)
    }

    #[test]
    fn top_level_tokens_preserve_literal_pieces_and_outer_trivia() {
        let (source, tree) = interpolation_fixture();
        let tokens: Vec<_> = tree.top_level_tokens(&source).collect();
        let expected = [
            "/// outer",
            "\n",
            "f",
            "(",
            "\"a",
            "b",
            "c",
            "\"",
            ")",
            " ",
            "// tail",
            "\n",
            "/// after",
            "\n",
            "z",
            "",
        ];
        assert_eq!(
            tokens.iter().map(|token| token.text()).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            tree.top_level_tokens(&source).rev().collect::<Vec<_>>(),
            tokens.iter().copied().rev().collect::<Vec<_>>()
        );
        for split in 0..=tokens.len() {
            let mut mixed = tree.top_level_tokens(&source);
            for expected in &tokens[..split] {
                assert_eq!(mixed.next(), Some(*expected));
            }
            let mut cloned = mixed.clone();
            for expected in tokens[split..].iter().rev() {
                assert_eq!(mixed.next_back(), Some(*expected));
            }
            for expected in &tokens[split..] {
                assert_eq!(cloned.next(), Some(*expected));
            }
            assert!(mixed.next().is_none());
            assert!(mixed.next_back().is_none());
            assert!(cloned.next().is_none());
        }
        for token in tree.tokens(&source) {
            let expected = tokens.contains(&token).then_some(token);
            assert_eq!(tree.top_level_token_at(&source, token.offset()), expected);
        }
        assert!(tree.top_level_token_at(&source, source.len() + 1).is_none());
        assert_eq!(
            tree.doc_blocks(&source)
                .map(|block| block.tokens().map(|token| token.text()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            [vec!["/// outer"], vec!["/// after"]]
        );
    }

    #[test]
    fn delimiters_do_not_pair_across_interpolation_boundaries() {
        let (source, tree) = interpolation_fixture();
        let token = |index| tree.token(&source, TokenId(index)).unwrap();
        for (open, close) in [(3, 28), (5, 18), (9, 16), (11, 14), (20, 23)] {
            assert_eq!(
                tree.closing_delimiter(&source, token(open).id()),
                Some(token(close))
            );
            assert_eq!(
                tree.opening_delimiter(&source, token(close).id()),
                Some(token(open))
            );
        }
        for open in [13, 17] {
            assert!(tree.closing_delimiter(&source, token(open).id()).is_none());
        }
        for close in [21, 22] {
            assert!(tree.opening_delimiter(&source, token(close).id()).is_none());
        }
    }
}
