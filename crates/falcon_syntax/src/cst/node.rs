use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

use super::kind::NodeKind;
use super::tree::{Element, NodeData, NodeId, SyntaxTree, TokenId};
use crate::ast::Span;
use crate::token::TokenKind;

#[derive(Clone, Copy)]
pub struct SyntaxNode<'a> {
    pub(crate) tree: &'a SyntaxTree,
    pub(crate) source: &'a str,
    pub(crate) id: NodeId,
}

impl<'a> SyntaxNode<'a> {
    fn data(self) -> &'a NodeData {
        &self.tree.nodes[self.id.0 as usize]
    }
    pub fn id(self) -> NodeId {
        self.id
    }
    pub fn tree(self) -> &'a SyntaxTree {
        self.tree
    }
    pub fn source(self) -> &'a str {
        self.source
    }
    pub fn kind(self) -> NodeKind {
        self.data().kind
    }
    pub fn span(self) -> Span {
        self.data().span
    }
    pub fn recovery_span(self) -> Option<Span> {
        let spans = &self.tree.recovery_spans;
        spans
            .binary_search_by_key(&self.id, |(node, _)| *node)
            .ok()
            .map(|index| spans[index].1)
    }
    pub fn full_span(self) -> Span {
        if self.id == NodeId(0) {
            return Span::new(0, self.source.len());
        }
        let mut span = self.span();
        if let Some(first) = self.first_token() {
            span.start = span.start.min(first.offset());
            let mut previous = first.prev();
            while let Some(token) = previous.filter(|token| token.is_trivia()) {
                span.start = token.offset();
                previous = token.prev();
            }
        }
        if let Some(last) = self.last_token() {
            span.end = span.end.max(last.span().end);
        }
        span
    }
    pub fn text(self) -> &'a str {
        let span = self.span();
        &self.source[span.start..span.end]
    }
    pub fn parent(self) -> Option<Self> {
        self.data().parent.map(|id| Self { id, ..self })
    }
    pub fn ancestors(self) -> impl Iterator<Item = Self> + Clone {
        std::iter::successors(Some(self), |node| node.parent())
    }
    pub fn children_with_tokens(self) -> SyntaxElementChildren<'a> {
        SyntaxElementChildren {
            parent: self,
            elements: self.tree.elements[self.data().children.clone()].iter(),
        }
    }
    pub fn children(self) -> SyntaxNodeChildren<'a> {
        SyntaxNodeChildren {
            inner: self.children_with_tokens(),
        }
    }
    pub fn descendants(self) -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator + Clone {
        (self.id.0 as usize..self.data().subtree_end).map(move |index| Self {
            id: NodeId(index as u32),
            ..self
        })
    }
    pub fn tokens(self) -> SyntaxTokens<'a> {
        SyntaxTokens::new(self.tree, self.source, self.data().tokens.clone())
    }
    pub fn significant_tokens(self) -> impl DoubleEndedIterator<Item = SyntaxToken<'a>> + Clone {
        self.tokens()
            .filter(|token| !token.is_trivia() && token.kind() != TokenKind::Eof)
    }
    pub fn first_token(self) -> Option<SyntaxToken<'a>> {
        self.tokens().next()
    }
    pub fn last_token(self) -> Option<SyntaxToken<'a>> {
        self.tokens().next_back()
    }
    pub fn prev_sibling(self) -> Option<Self> {
        self.parent()?
            .children()
            .take_while(|node| node.id != self.id)
            .last()
    }
    pub fn next_sibling(self) -> Option<Self> {
        let mut children = self.parent()?.children();
        children.find(|node| node.id == self.id)?;
        children.next()
    }
    pub fn child_token(self, kind: TokenKind) -> Option<SyntaxToken<'a>> {
        self.children_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .find(|token| token.kind() == kind)
    }
}

impl fmt::Debug for SyntaxNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxNode")
            .field("kind", &self.kind())
            .field("span", &self.span())
            .finish()
    }
}

impl PartialEq for SyntaxNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.id == other.id
    }
}
impl Eq for SyntaxNode<'_> {}

#[derive(Clone, Copy)]
pub struct SyntaxToken<'a> {
    pub(crate) tree: &'a SyntaxTree,
    pub(crate) source: &'a str,
    pub(crate) id: TokenId,
}

impl<'a> SyntaxToken<'a> {
    pub fn id(self) -> TokenId {
        self.id
    }
    pub fn kind(self) -> TokenKind {
        self.tree.tokens[self.id.index()].kind
    }
    pub fn text(self) -> &'a str {
        self.tree.tokens[self.id.index()].text(self.source)
    }
    pub fn offset(self) -> usize {
        self.tree.tokens[self.id.index()].offset()
    }
    pub fn len(self) -> usize {
        self.tree.tokens[self.id.index()].len()
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn span(self) -> Span {
        Span::new(self.offset(), self.offset() + self.len())
    }
    pub fn is_trivia(self) -> bool {
        self.tree.tokens[self.id.index()].is_trivia()
    }
    pub fn parent(self) -> SyntaxNode<'a> {
        SyntaxNode {
            tree: self.tree,
            source: self.source,
            id: self.tree.token_parents[self.id.index()],
        }
    }
    pub fn next(self) -> Option<Self> {
        self.tree
            .token(self.source, TokenId(self.id.0.checked_add(1)?))
    }
    pub fn prev(self) -> Option<Self> {
        self.tree
            .token(self.source, TokenId(self.id.0.checked_sub(1)?))
    }
    pub fn next_significant(self) -> Option<Self> {
        std::iter::successors(self.next(), |token| token.next())
            .find(|token| !token.is_trivia() && token.kind() != TokenKind::Eof)
    }
    pub fn prev_significant(self) -> Option<Self> {
        std::iter::successors(self.prev(), |token| token.prev())
            .find(|token| !token.is_trivia() && token.kind() != TokenKind::Eof)
    }
}

impl fmt::Debug for SyntaxToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxToken")
            .field("kind", &self.kind())
            .field("span", &self.span())
            .field("text", &self.text())
            .finish()
    }
}
impl PartialEq for SyntaxToken<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.id == other.id
    }
}
impl Eq for SyntaxToken<'_> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxToken<'a>),
}

impl<'a> SyntaxElement<'a> {
    pub fn into_node(self) -> Option<SyntaxNode<'a>> {
        if let Self::Node(node) = self {
            Some(node)
        } else {
            None
        }
    }
    pub fn into_token(self) -> Option<SyntaxToken<'a>> {
        if let Self::Token(token) = self {
            Some(token)
        } else {
            None
        }
    }
    pub fn span(self) -> Span {
        match self {
            Self::Node(node) => node.span(),
            Self::Token(token) => token.span(),
        }
    }
    pub fn text(self) -> &'a str {
        match self {
            Self::Node(node) => node.text(),
            Self::Token(token) => token.text(),
        }
    }
}

#[derive(Clone)]
pub struct SyntaxElementChildren<'a> {
    parent: SyntaxNode<'a>,
    elements: std::slice::Iter<'a, Element>,
}
impl<'a> SyntaxElementChildren<'a> {
    fn resolve(&self, element: Element) -> SyntaxElement<'a> {
        if let Some(id) = element.as_node() {
            SyntaxElement::Node(SyntaxNode { id, ..self.parent })
        } else {
            SyntaxElement::Token(SyntaxToken {
                tree: self.parent.tree,
                source: self.parent.source,
                id: element.as_token().unwrap(),
            })
        }
    }
}
impl<'a> Iterator for SyntaxElementChildren<'a> {
    type Item = SyntaxElement<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let element = *self.elements.next()?;
        Some(self.resolve(element))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.elements.size_hint()
    }
}
impl DoubleEndedIterator for SyntaxElementChildren<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let element = *self.elements.next_back()?;
        Some(self.resolve(element))
    }
}
impl ExactSizeIterator for SyntaxElementChildren<'_> {}
impl std::iter::FusedIterator for SyntaxElementChildren<'_> {}

#[derive(Clone)]
pub struct SyntaxNodeChildren<'a> {
    inner: SyntaxElementChildren<'a>,
}
// Hand-rolled loops: every tree walk funnels through these, and nested
// `find_map` adapters copy each child through several layers in debug builds.
impl<'a> Iterator for SyntaxNodeChildren<'a> {
    type Item = SyntaxNode<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        for element in self.inner.elements.by_ref() {
            if let Some(id) = element.as_node() {
                return Some(SyntaxNode {
                    id,
                    ..self.inner.parent
                });
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.inner.size_hint().1)
    }
}
impl DoubleEndedIterator for SyntaxNodeChildren<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(element) = self.inner.elements.next_back() {
            if let Some(id) = element.as_node() {
                return Some(SyntaxNode {
                    id,
                    ..self.inner.parent
                });
            }
        }
        None
    }
}
impl std::iter::FusedIterator for SyntaxNodeChildren<'_> {}

#[derive(Debug, Clone)]
pub struct SyntaxTokens<'a> {
    tree: &'a SyntaxTree,
    source: &'a str,
    range: Range<usize>,
}
impl<'a> SyntaxTokens<'a> {
    pub(crate) fn new(tree: &'a SyntaxTree, source: &'a str, range: Range<usize>) -> Self {
        Self {
            tree,
            source,
            range,
        }
    }
    fn resolve(&self, index: usize) -> SyntaxToken<'a> {
        SyntaxToken {
            tree: self.tree,
            source: self.source,
            id: TokenId(index as u32),
        }
    }
}
impl<'a> Iterator for SyntaxTokens<'a> {
    type Item = SyntaxToken<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(self.resolve(index))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl DoubleEndedIterator for SyntaxTokens<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let index = self.range.next_back()?;
        Some(self.resolve(index))
    }
}
impl ExactSizeIterator for SyntaxTokens<'_> {}
impl std::iter::FusedIterator for SyntaxTokens<'_> {}

pub trait AstNode<'a>: Copy {
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self>;
    fn syntax(self) -> SyntaxNode<'a>;
    fn span(self) -> Span {
        self.syntax().span()
    }
}

#[derive(Clone)]
pub struct AstChildren<'a, T> {
    children: SyntaxNodeChildren<'a>,
    marker: PhantomData<T>,
}
impl<'a, T: AstNode<'a>> AstChildren<'a, T> {
    pub fn empty() -> Self {
        static EMPTY: std::sync::LazyLock<SyntaxTree> =
            std::sync::LazyLock::new(|| SyntaxTree::build("", &[], Vec::new()));
        Self::new(EMPTY.root(""))
    }
    pub fn new(parent: SyntaxNode<'a>) -> Self {
        Self::from_children(parent.children())
    }
    pub fn from_children(children: SyntaxNodeChildren<'a>) -> Self {
        Self {
            children,
            marker: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.clone().count()
    }
    pub fn is_empty(&self) -> bool {
        self.clone().next().is_none()
    }
    pub fn first(&self) -> Option<T> {
        self.clone().next()
    }
    pub fn last(&self) -> Option<T> {
        self.clone().next_back()
    }
}
impl<'a, T: AstNode<'a>> Iterator for AstChildren<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        for node in self.children.by_ref() {
            if let Some(node) = T::cast(node) {
                return Some(node);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.children.size_hint()
    }
}
impl<'a, T: AstNode<'a>> DoubleEndedIterator for AstChildren<'a, T> {
    fn next_back(&mut self) -> Option<T> {
        while let Some(node) = self.children.next_back() {
            if let Some(node) = T::cast(node) {
                return Some(node);
            }
        }
        None
    }
}
impl<'a, T: AstNode<'a>> std::iter::FusedIterator for AstChildren<'a, T> {}
impl<'a, T: AstNode<'a> + fmt::Debug> fmt::Debug for AstChildren<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AstChildren;
    use crate::cst::Expr;

    #[test]
    fn empty_children_are_empty_from_both_ends() {
        let mut children = AstChildren::<Expr<'_>>::empty();
        assert!(children.is_empty());
        assert_eq!(children.len(), 0);
        assert!(children.first().is_none());
        assert!(AstChildren::last(&children).is_none());
        assert!(children.next().is_none());
        assert!(children.next_back().is_none());
        assert!(children.next().is_none());
        assert_eq!(children.clone().count(), 0);
    }
}
