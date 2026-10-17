use std::num::NonZeroU32;
use std::ops::Range;
use std::sync::OnceLock;

use super::Span;
use super::kind::NodeKind;
use super::node::SyntaxNode;
use crate::token::{Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

/// An arena link encoded as index + 1 so optional links occupy one `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeLink(NonZeroU32);

impl NodeLink {
    fn new(id: NodeId) -> Self {
        Self(NonZeroU32::new(id.0.checked_add(1).expect("syntax node capacity exceeded")).unwrap())
    }

    pub(crate) fn id(self) -> NodeId {
        NodeId(self.0.get() - 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenId(pub u32);

impl TokenId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Start {
        kind: NodeKind,
        forward_parent: Option<usize>,
        offset: usize,
    },
    Token {
        kind: TokenKind,
        len: usize,
        offset: usize,
    },
    Finish,
    RecoverySpan(Span),
    Tombstone,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Element(u32);

impl Element {
    pub(crate) fn node(id: NodeId) -> Self {
        assert!(id.0 < 1 << 31);
        Self(id.0)
    }
    pub(crate) fn token(id: TokenId) -> Self {
        assert!(id.0 < 1 << 31);
        Self(id.0 | 1 << 31)
    }
    pub(crate) fn as_node(self) -> Option<NodeId> {
        (self.0 >> 31 == 0).then_some(NodeId(self.0))
    }
    pub(crate) fn as_token(self) -> Option<TokenId> {
        (self.0 >> 31 != 0).then_some(TokenId(self.0 & !(1 << 31)))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NodeData {
    pub(crate) kind: NodeKind,
    pub(crate) parent: Option<NodeLink>,
    pub(crate) previous_sibling: Option<NodeLink>,
    // Element IDs use 31 bits; even the combined node/token edge range fits in u32.
    // Keep source spans and public view ranges in native-width offsets.
    pub(crate) children: Range<u32>,
    pub(crate) tokens: Range<u32>,
    pub(crate) span: Span,
    pub(crate) subtree_end: u32,
    flags: u8,
}

impl NodeData {
    const SIGNIFICANT: u8 = 1;
    const OPTIONAL_PARAM: u8 = 2;
    const NAMED_PARAM: u8 = 4;
    const REQUIRED_PARAM: u8 = 8;
    const HAS_PARAMS: u8 = 16;

    pub(crate) fn param_kind(&self) -> super::ParamKind {
        if self.flags & Self::NAMED_PARAM != 0 {
            super::ParamKind::Named {
                required: self.flags & Self::REQUIRED_PARAM != 0,
            }
        } else if self.flags & Self::OPTIONAL_PARAM != 0 {
            super::ParamKind::OptionalPositional
        } else {
            super::ParamKind::Positional
        }
    }

    pub(crate) fn child_range(&self) -> Range<usize> {
        self.children.start as usize..self.children.end as usize
    }

    pub(crate) fn token_range(&self) -> Range<usize> {
        self.tokens.start as usize..self.tokens.end as usize
    }
}

/// A lossless syntax arena. Source text is supplied separately when creating views.
#[derive(Debug, Clone)]
pub struct SyntaxTree {
    pub(crate) source_len: usize,
    pub(crate) nodes: Box<[NodeData]>,
    pub(crate) elements: Box<[Element]>,
    pub(crate) tokens: Box<[Token]>,
    pub(crate) token_parents: Box<[NodeId]>,
    pub(crate) delimiter_pairs: OnceLock<Box<[u32]>>,
    pub(crate) recovery_spans: Box<[(NodeId, Span)]>,
}

impl SyntaxTree {
    pub fn root<'a>(&'a self, source: &'a str) -> SyntaxNode<'a> {
        debug_assert_eq!(source.len(), self.source_len);
        SyntaxNode {
            tree: self,
            source,
            id: NodeId(0),
        }
    }

    pub fn program<'a>(&'a self, source: &'a str) -> super::Program<'a> {
        super::Program::cast(self.root(source)).expect("syntax tree root must be Program")
    }

    pub fn node<'a>(&'a self, source: &'a str, id: NodeId) -> Option<SyntaxNode<'a>> {
        debug_assert_eq!(source.len(), self.source_len);
        self.nodes.get(id.0 as usize)?;
        Some(SyntaxNode {
            tree: self,
            source,
            id,
        })
    }

    pub fn is_lossless(&self, source: &str) -> bool {
        if source.len() != self.source_len {
            return false;
        }
        let mut offset = 0;
        for token in &self.tokens {
            if token.offset() != offset {
                return false;
            }
            let Some(end) = offset.checked_add(token.len()) else {
                return false;
            };
            if source.get(offset..end).is_none() {
                return false;
            }
            offset = end;
        }
        offset == source.len()
            && self
                .tokens
                .last()
                .is_some_and(|token| token.kind == TokenKind::Eof)
    }

    pub fn build(source: &str, tokens: &[Token], mut events: Vec<Event>) -> Self {
        let node_capacity = events
            .iter()
            .filter(|event| matches!(event, Event::Start { .. }))
            .count()
            .max(1);
        let token_capacity = tokens.len()
            + events
                .iter()
                .filter(|event| matches!(event, Event::Token { .. }))
                .count()
            + 1;
        let mut sink = Sink {
            source_len: source.len(),
            lexical: tokens,
            lexical_index: 0,
            offset: 0,
            nodes: Vec::with_capacity(node_capacity),
            tokens: Vec::with_capacity(token_capacity),
            parents: Vec::with_capacity(token_capacity),
            edges: Vec::with_capacity(node_capacity + token_capacity),
            stack: Vec::new(),
        };
        let mut kinds = Vec::new();
        let mut recovery_spans = Vec::new();
        for index in 0..events.len() {
            match std::mem::replace(&mut events[index], Event::Tombstone) {
                Event::Start {
                    kind,
                    forward_parent,
                    offset,
                } => {
                    if !sink.stack.is_empty() {
                        sink.flush(offset);
                    }
                    kinds.push(kind);
                    let mut next = forward_parent;
                    let mut current = index;
                    while let Some(distance) = next {
                        assert!(distance > 0, "forward parent must follow its child");
                        current += distance;
                        match std::mem::replace(&mut events[current], Event::Tombstone) {
                            Event::Start {
                                kind,
                                forward_parent,
                                ..
                            } => {
                                kinds.push(kind);
                                next = forward_parent;
                            }
                            Event::Tombstone => break,
                            _ => panic!("forward parent must be a start event"),
                        }
                    }
                    for kind in kinds.drain(..).rev() {
                        sink.start(kind, offset);
                    }
                }
                Event::Token { kind, len, offset } => {
                    sink.flush(offset);
                    assert_eq!(sink.offset, offset, "token events must be in source order");
                    assert!(offset + len <= source.len());
                    if kind != TokenKind::Eof {
                        sink.emit(Token::new(kind, offset, len));
                        sink.offset += len;
                    }
                }
                Event::Finish => {
                    if sink.stack.len() == 1 {
                        sink.flush(source.len());
                        sink.emit_eof();
                    }
                    sink.finish();
                }
                Event::RecoverySpan(span) => {
                    recovery_spans.push((
                        sink.stack.last().expect("recovery span requires a node").id,
                        span,
                    ));
                }
                Event::Tombstone => {}
            }
        }
        if sink.nodes.is_empty() {
            sink.start(NodeKind::Program, 0);
            sink.flush(source.len());
            sink.emit_eof();
            sink.finish();
        }
        assert!(sink.stack.is_empty(), "unclosed syntax events");
        assert_eq!(sink.offset, source.len(), "root must consume the source");
        let mut cursor = 0u32;
        for node in &mut sink.nodes {
            let end = cursor
                .checked_add(node.children.end)
                .expect("syntax element capacity exceeded");
            node.children = cursor..end;
            cursor = end;
        }
        let mut elements = vec![Element::node(NodeId(0)); cursor as usize];
        let mut positions: Vec<_> = sink.nodes.iter().map(|node| node.children.start).collect();
        for (parent, element) in sink.edges {
            let position = &mut positions[parent.0 as usize];
            elements[*position as usize] = element;
            *position = position
                .checked_add(1)
                .expect("syntax element capacity exceeded");
        }
        cache_param_kinds(&mut sink.nodes, &elements, &sink.tokens);
        recovery_spans.sort_unstable_by_key(|(node, _)| *node);
        Self {
            source_len: source.len(),
            nodes: sink.nodes.into_boxed_slice(),
            elements: elements.into_boxed_slice(),
            tokens: sink.tokens.into_boxed_slice(),
            token_parents: sink.parents.into_boxed_slice(),
            delimiter_pairs: OnceLock::new(),
            recovery_spans: recovery_spans.into_boxed_slice(),
        }
    }
}

fn cache_param_kinds(nodes: &mut [NodeData], elements: &[Element], tokens: &[Token]) {
    for parent in 0..nodes.len() {
        if nodes[parent].flags & NodeData::HAS_PARAMS == 0 {
            continue;
        }
        let children = &elements[nodes[parent].child_range()];
        let mut cursor = 0;
        let mut group = 0;
        let mut previous_start = 0;
        for child in children.iter().filter_map(|element| element.as_node()) {
            let param = &mut nodes[child.0 as usize];
            if param.kind != NodeKind::FormalParam {
                continue;
            }
            // Only markers strictly before param.span.start apply. Empty recovery
            // params can precede earlier siblings, so restart their marker scan.
            if param.span.start < previous_start {
                cursor = 0;
                group = 0;
            }
            previous_start = param.span.start;
            while let Some(element) = children.get(cursor) {
                if let Some(token) = element.as_token() {
                    let token = &tokens[token.index()];
                    if token.offset() >= param.span.start {
                        break;
                    }
                    match token.kind {
                        TokenKind::LBracket => group = NodeData::OPTIONAL_PARAM,
                        TokenKind::LBrace => group = NodeData::NAMED_PARAM,
                        _ => {}
                    }
                }
                cursor += 1;
            }
            param.flags |= group;
        }
    }
}

struct Sink<'a> {
    source_len: usize,
    lexical: &'a [Token],
    lexical_index: usize,
    offset: usize,
    nodes: Vec<NodeData>,
    tokens: Vec<Token>,
    parents: Vec<NodeId>,
    edges: Vec<(NodeId, Element)>,
    stack: Vec<OpenNode>,
}

struct OpenNode {
    id: NodeId,
    last_child: Option<NodeLink>,
}

impl Sink<'_> {
    fn start(&mut self, kind: NodeKind, offset: usize) {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("syntax node capacity exceeded"));
        let (parent, previous_sibling) = if let Some(parent) = self.stack.last_mut() {
            let previous = parent.last_child.replace(NodeLink::new(id));
            (Some(parent.id), previous)
        } else {
            (None, None)
        };
        if let Some(parent) = parent {
            if kind == NodeKind::FormalParam {
                self.nodes[parent.0 as usize].flags |= NodeData::HAS_PARAMS;
            }
            self.edge(parent, Element::node(id));
        } else {
            assert!(self.nodes.is_empty(), "syntax tree must have one root");
        }
        let token_start = u32::try_from(self.tokens.len()).expect("syntax token capacity exceeded");
        self.nodes.push(NodeData {
            kind,
            parent: parent.map(NodeLink::new),
            previous_sibling,
            children: 0..0,
            tokens: token_start..token_start,
            span: Span::new(offset, offset),
            subtree_end: 0,
            flags: 0,
        });
        self.stack.push(OpenNode {
            id,
            last_child: None,
        });
    }

    fn edge(&mut self, parent: NodeId, element: Element) {
        let count = &mut self.nodes[parent.0 as usize].children.end;
        *count = count
            .checked_add(1)
            .expect("syntax element capacity exceeded");
        self.edges.push((parent, element));
    }

    fn emit(&mut self, token: Token) {
        let parent = self.stack.last().expect("tokens require an open node").id;
        let id = TokenId(u32::try_from(self.tokens.len()).expect("syntax token capacity exceeded"));
        self.edge(parent, Element::token(id));
        self.tokens.push(token);
        self.parents.push(parent);
        let node = &mut self.nodes[parent.0 as usize];
        if token.kind == TokenKind::Required && node.kind == NodeKind::FormalParam {
            node.flags |= NodeData::REQUIRED_PARAM;
        }
        if !token.is_trivia() && token.kind != TokenKind::Eof {
            if node.flags & NodeData::SIGNIFICANT == 0 {
                node.span.start = token.offset();
            }
            node.span.end = token.offset() + token.len();
            node.flags |= NodeData::SIGNIFICANT;
        }
    }

    fn flush(&mut self, end: usize) {
        assert!(end <= self.source_len);
        while self.offset < end {
            while self
                .lexical
                .get(self.lexical_index)
                .is_some_and(|token| token.offset() + token.len() <= self.offset)
            {
                self.lexical_index += 1;
            }
            let token = self.lexical.get(self.lexical_index);
            let (kind, until) = match token {
                Some(token) if token.offset() <= self.offset => {
                    (token.kind, (token.offset() + token.len()).min(end))
                }
                // Canonical gaps are lexer-ignored bytes, not skipped significant tokens.
                Some(token) => (TokenKind::Whitespace, token.offset().min(end)),
                None => (TokenKind::Whitespace, end),
            };
            self.emit(Token::new(kind, self.offset, until - self.offset));
            self.offset = until;
        }
    }

    fn emit_eof(&mut self) {
        self.emit(Token::new(TokenKind::Eof, self.source_len, 0));
    }

    fn finish(&mut self) {
        let id = self.stack.pop().expect("finish requires an open node").id;
        let subtree_end = u32::try_from(self.nodes.len()).expect("syntax node capacity exceeded");
        let node = &mut self.nodes[id.0 as usize];
        node.tokens.end = u32::try_from(self.tokens.len()).expect("syntax token capacity exceeded");
        node.subtree_end = subtree_end;
        let (span, significant) = (node.span, node.flags & NodeData::SIGNIFICANT != 0);
        if significant && let Some(parent) = self.stack.last() {
            let parent = &mut self.nodes[parent.id.0 as usize];
            if parent.flags & NodeData::SIGNIFICANT == 0 {
                parent.span.start = span.start;
            }
            parent.span.end = span.end;
            parent.flags |= NodeData::SIGNIFICANT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::node::{AstChildren, AstNode, SyntaxElement};
    use super::*;

    fn start(kind: NodeKind, offset: usize) -> Event {
        Event::Start {
            kind,
            offset,
            forward_parent: None,
        }
    }

    #[test]
    fn optional_node_links_are_compact_and_preserve_indices() {
        assert_eq!(std::mem::size_of::<Option<NodeLink>>(), 4);
        assert_eq!(NodeLink::new(NodeId(0)).id(), NodeId(0));
        let last = NodeId((1 << 31) - 1);
        assert_eq!(NodeLink::new(last).id(), last);
    }

    #[test]
    fn arena_ranges_are_compact_and_preserve_endpoints() {
        assert!(std::mem::size_of::<NodeData>() <= 48);
        let node = NodeData {
            kind: NodeKind::Program,
            parent: None,
            previous_sibling: None,
            children: 0..u32::MAX,
            tokens: u32::MAX..u32::MAX,
            span: Span::new(0, 0),
            subtree_end: 1,
            flags: 0,
        };
        assert_eq!(node.child_range(), 0..u32::MAX as usize);
        assert_eq!(node.token_range(), u32::MAX as usize..u32::MAX as usize);
    }

    fn assert_param_cache(events: Vec<Event>, source_len: usize) -> Vec<super::super::ParamKind> {
        use super::super::{FormalParam, ParamKind};

        let source = "x".repeat(source_len);
        let tree = SyntaxTree::build(&source, &[], events);
        tree.root(&source)
            .descendants()
            .filter_map(FormalParam::cast)
            .map(|param| {
                let node = param.syntax();
                let mut expected = ParamKind::Positional;
                if let Some(parent) = node.parent() {
                    for token in parent
                        .children_with_tokens()
                        .filter_map(SyntaxElement::into_token)
                        .filter(|token| token.span().start < param.span().start)
                    {
                        match token.kind() {
                            TokenKind::LBracket => expected = ParamKind::OptionalPositional,
                            TokenKind::LBrace => expected = ParamKind::Named { required: false },
                            _ => {}
                        }
                    }
                }
                if matches!(expected, ParamKind::Named { .. }) {
                    expected = ParamKind::Named {
                        required: node
                            .children_with_tokens()
                            .filter_map(SyntaxElement::into_token)
                            .any(|token| token.kind() == TokenKind::Required),
                    };
                }
                assert_eq!(param.kind(), expected, "parameter {:?}", param.span());
                for kind in [TokenKind::Required, TokenKind::Ident, TokenKind::LBrace] {
                    assert_eq!(
                        node.child_token(kind),
                        node.children_with_tokens()
                            .filter_map(SyntaxElement::into_token)
                            .find(|token| token.kind() == kind)
                    );
                }
                expected
            })
            .collect()
    }

    fn param_token(events: &mut Vec<Event>, offset: &mut usize, kind: TokenKind) {
        events.push(Event::Token {
            kind,
            len: 1,
            offset: *offset,
        });
        *offset += 1;
    }

    fn param_fixture(
        events: &mut Vec<Event>,
        offset: &mut usize,
        direct_required: bool,
        nested_required: bool,
    ) {
        events.push(start(NodeKind::FormalParam, *offset));
        if direct_required {
            param_token(events, offset, TokenKind::Required);
        }
        if nested_required {
            events.push(start(NodeKind::Error, *offset));
            param_token(events, offset, TokenKind::Required);
            param_token(events, offset, TokenKind::LBrace);
            events.push(Event::Finish);
        }
        param_token(events, offset, TokenKind::Ident);
        events.push(Event::Finish);
    }

    #[test]
    fn parameter_cache_matches_scanning_for_valid_and_malformed_groups() {
        let markers = [
            TokenKind::LBracket,
            TokenKind::LBrace,
            TokenKind::RBracket,
            TokenKind::RBrace,
            TokenKind::Comma,
            TokenKind::Required,
            TokenKind::Whitespace,
        ];
        for first in markers {
            for second in markers {
                let mut events = vec![
                    start(NodeKind::Program, 0),
                    start(NodeKind::FormalParamList, 0),
                ];
                let mut offset = 0;
                param_fixture(&mut events, &mut offset, false, false);
                for marker in [first, second] {
                    // Empty params share the following marker's offset: it must
                    // not affect them, even though the scan sees later edges.
                    events.push(start(NodeKind::FormalParam, offset));
                    events.push(Event::Finish);
                    param_token(&mut events, &mut offset, marker);
                    for (direct, nested) in [(false, false), (true, false), (false, true)] {
                        param_fixture(&mut events, &mut offset, direct, nested);
                    }
                }
                events.extend([Event::Finish, Event::Finish]);
                assert_eq!(assert_param_cache(events, offset).len(), 9);
            }
        }
    }

    #[test]
    fn parameter_cache_preserves_empty_recovery_offsets_and_direct_parent_scope() {
        use super::super::ParamKind;

        let mut events = vec![start(NodeKind::Program, 0)];
        let mut offset = 0;
        param_token(&mut events, &mut offset, TokenKind::LBrace);
        param_fixture(&mut events, &mut offset, true, false);
        // A recovered empty node may point before already-consumed markers.
        events.push(start(NodeKind::FormalParam, 0));
        events.push(Event::Finish);
        events.push(start(NodeKind::Error, offset));
        param_fixture(&mut events, &mut offset, true, false);
        events.push(Event::Finish);
        param_fixture(&mut events, &mut offset, false, true);
        events.push(Event::Finish);
        assert_eq!(
            assert_param_cache(events, offset),
            [
                ParamKind::Named { required: true },
                ParamKind::Positional,
                ParamKind::Positional,
                ParamKind::Named { required: false },
            ]
        );
    }

    #[test]
    fn parentless_parameter_cache_remains_positional() {
        assert_eq!(
            assert_param_cache(
                vec![
                    start(NodeKind::FormalParam, 0),
                    Event::Token {
                        kind: TokenKind::Required,
                        len: 1,
                        offset: 0,
                    },
                    Event::Finish,
                ],
                1,
            ),
            [super::super::ParamKind::Positional]
        );
    }

    fn sink_with_child_count(count: u32) -> Sink<'static> {
        let mut sink = Sink {
            source_len: 0,
            lexical: &[],
            lexical_index: 0,
            offset: 0,
            nodes: Vec::new(),
            tokens: Vec::new(),
            parents: Vec::new(),
            edges: Vec::new(),
            stack: Vec::new(),
        };
        sink.start(NodeKind::Program, 0);
        sink.nodes[0].children.end = count;
        sink
    }

    #[test]
    fn child_count_accepts_maximum() {
        let mut sink = sink_with_child_count(u32::MAX - 1);
        sink.edge(NodeId(0), Element::node(NodeId(0)));
        assert_eq!(sink.nodes[0].children.end, u32::MAX);
    }

    #[test]
    #[should_panic(expected = "syntax element capacity exceeded")]
    fn child_count_rejects_overflow() {
        let mut sink = sink_with_child_count(u32::MAX);
        sink.edge(NodeId(0), Element::node(NodeId(0)));
    }

    #[test]
    fn node_lookup_preserves_arena_identity() {
        let source = "";
        let tree = SyntaxTree::build(
            source,
            &[Token::new(TokenKind::Eof, 0, 0)],
            vec![
                start(NodeKind::Program, 0),
                start(NodeKind::Identifier, 0),
                Event::Finish,
                Event::Finish,
            ],
        );
        let child = tree.root(source).children().next().unwrap();
        assert_eq!(tree.node(source, child.id()), Some(child));
        assert_eq!(tree.node(source, NodeId(0)), Some(tree.root(source)));
        assert_eq!(tree.node(source, NodeId(2)), None);
        assert_eq!(tree.root(source).parent(), None);
        assert_eq!(child.parent(), Some(tree.root(source)));
    }

    #[test]
    fn siblings_skip_descendants_of_zero_width_nodes() {
        let source = "";
        let tree = SyntaxTree::build(
            source,
            &[Token::new(TokenKind::Eof, 0, 0)],
            vec![
                start(NodeKind::Program, 0),
                start(NodeKind::Error, 0),
                start(NodeKind::Identifier, 0),
                Event::Finish,
                Event::Finish,
                start(NodeKind::Error, 0),
                start(NodeKind::Identifier, 0),
                Event::Finish,
                Event::Finish,
                Event::Finish,
            ],
        );
        let mut children = tree.root(source).children();
        let first = children.next().unwrap();
        let second = children.next().unwrap();
        let nested = first.children().next().unwrap();
        let second_nested = second.children().next().unwrap();
        assert_eq!(first.prev_sibling(), None);
        assert_eq!(first.next_sibling(), Some(second));
        assert_eq!(second.prev_sibling(), Some(first));
        assert_eq!(second.next_sibling(), None);
        assert_eq!(nested.prev_sibling(), None);
        assert_eq!(nested.next_sibling(), None);
        assert_eq!(second_nested.prev_sibling(), None);
        assert_eq!(second_nested.next_sibling(), None);
        assert_eq!(nested.parent(), Some(first));
        assert_eq!(second_nested.parent(), Some(second));
    }

    #[test]
    fn forward_parent_chain_preserves_prior_sibling() {
        let tree = SyntaxTree::build(
            "",
            &[Token::new(TokenKind::Eof, 0, 0)],
            vec![
                start(NodeKind::Program, 0),
                start(NodeKind::Error, 0),
                Event::Finish,
                Event::Start {
                    kind: NodeKind::Identifier,
                    offset: 0,
                    forward_parent: Some(2),
                },
                Event::Finish,
                Event::Start {
                    kind: NodeKind::Error,
                    offset: 0,
                    forward_parent: Some(2),
                },
                Event::Finish,
                start(NodeKind::Error, 0),
                Event::Finish,
                Event::Finish,
            ],
        );
        let mut siblings = tree.root("").children();
        let earlier = siblings.next().unwrap();
        let outer = siblings.next().unwrap();
        let inner = outer.children().next().unwrap();
        let identifier = inner.children().next().unwrap();
        assert!(siblings.next().is_none());
        assert_eq!(outer.prev_sibling(), Some(earlier));
        assert_eq!(earlier.next_sibling(), Some(outer));
        assert_eq!(inner.prev_sibling(), None);
        assert_eq!(inner.next_sibling(), None);
        assert_eq!(identifier.parent(), Some(inner));
        assert_eq!(identifier.prev_sibling(), None);
    }

    #[test]
    fn forward_parents_recovery_split_tokens_and_navigation() {
        let source = "/// doc\n a + ?> >  ";
        let lexical = [
            Token::new(TokenKind::DocComment, 0, 7),
            Token::new(TokenKind::Newline, 7, 1),
            Token::new(TokenKind::Whitespace, 8, 1),
            Token::new(TokenKind::Ident, 9, 1),
            Token::new(TokenKind::Whitespace, 10, 1),
            Token::new(TokenKind::Plus, 11, 1),
            Token::new(TokenKind::Whitespace, 12, 1),
            Token::new(TokenKind::Qmark, 13, 1),
            Token::new(TokenKind::Gt, 14, 1),
            Token::new(TokenKind::Whitespace, 15, 1),
            Token::new(TokenKind::Gt, 16, 1),
            Token::new(TokenKind::Whitespace, 17, 2),
            Token::new(TokenKind::Eof, 19, 0),
        ];
        let tree = SyntaxTree::build(
            source,
            &lexical,
            vec![
                start(NodeKind::Program, 0),
                Event::Start {
                    kind: NodeKind::Identifier,
                    offset: 9,
                    forward_parent: Some(3),
                },
                Event::Token {
                    kind: TokenKind::Ident,
                    offset: 9,
                    len: 1,
                },
                Event::Finish,
                start(NodeKind::Error, 11),
                Event::Token {
                    kind: TokenKind::Plus,
                    offset: 11,
                    len: 1,
                },
                start(NodeKind::Error, 14),
                Event::Finish,
                Event::Token {
                    kind: TokenKind::Gt,
                    offset: 14,
                    len: 1,
                },
                Event::Finish,
                Event::Finish,
            ],
        );
        assert!(tree.is_lossless(source));
        assert_eq!(
            tree.tokens(source)
                .map(|token| token.text())
                .collect::<String>(),
            source
        );
        let root = tree.root(source);
        let outer = root.children().next().unwrap();
        assert_eq!(outer.kind(), NodeKind::Error);
        assert_eq!(outer.span(), Span::new(9, 15));
        assert_eq!(outer.full_span().start, 0);
        let mut children = outer.children();
        let identifier = children.next().unwrap();
        let missing = children.next().unwrap();
        assert_eq!(missing.span(), Span::new(14, 14));
        assert!(missing.tokens().next().is_none());
        assert_eq!(identifier.next_sibling(), Some(missing));
        assert_eq!(missing.prev_sibling(), Some(identifier));
        assert_eq!(identifier.prev_sibling(), None);
        assert_eq!(missing.next_sibling(), None);
        assert_eq!(root.prev_sibling(), None);
        assert_eq!(root.next_sibling(), None);
        assert_eq!(outer.prev_sibling(), None);
        assert_eq!(outer.next_sibling(), None);
        assert_eq!(identifier.parent(), Some(outer));
        assert_eq!(identifier.ancestors().count(), 3);
        assert_eq!(root.descendants().count(), 4);
        assert_eq!(outer.child_token(TokenKind::Qmark).unwrap().text(), "?");
        assert!(
            root.children_with_tokens()
                .next()
                .unwrap()
                .into_token()
                .is_some()
        );
        assert_eq!(identifier.first_token(), identifier.last_token());
        let name = identifier.first_token().unwrap();
        assert_eq!(name.parent(), identifier);
        assert_eq!(name.next_significant().unwrap().kind(), TokenKind::Plus);
        assert_eq!(
            name.next_significant().unwrap().prev_significant(),
            Some(name)
        );
        assert_eq!(SyntaxElement::Node(identifier).text(), "a");
        assert_eq!(root.tokens().len(), tree.tokens(source).len());

        #[derive(Clone, Copy)]
        struct ErrorView<'a>(SyntaxNode<'a>);
        impl<'a> AstNode<'a> for ErrorView<'a> {
            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                (node.kind() == NodeKind::Error).then_some(Self(node))
            }
            fn syntax(self) -> SyntaxNode<'a> {
                self.0
            }
        }
        let views = AstChildren::<ErrorView<'_>>::new(outer);
        assert_eq!(views.len(), 1);
        assert!(!views.is_empty());
        assert_eq!(views.first().unwrap().span(), missing.span());
        assert_eq!(views.last().unwrap().syntax(), missing);
    }

    #[test]
    fn one_lexical_shift_can_emit_two_tokens() {
        let source = ">>";
        let lexical = [Token::new(TokenKind::GtGt, 0, 2)];
        let tree = SyntaxTree::build(
            source,
            &lexical,
            vec![
                start(NodeKind::Program, 0),
                Event::Token {
                    kind: TokenKind::Gt,
                    offset: 0,
                    len: 1,
                },
                Event::Token {
                    kind: TokenKind::Gt,
                    offset: 1,
                    len: 1,
                },
                Event::Finish,
            ],
        );
        assert!(tree.is_lossless(source));
        assert_eq!(
            tree.significant_tokens(source)
                .map(|token| token.kind())
                .collect::<Vec<_>>(),
            [TokenKind::Gt, TokenKind::Gt]
        );
    }

    #[test]
    fn abandoned_forward_parent_preserves_completed_child() {
        let source = "x";
        let lexical = [Token::new(TokenKind::Ident, 0, 1)];
        let tree = SyntaxTree::build(
            source,
            &lexical,
            vec![
                start(NodeKind::Program, 0),
                Event::Start {
                    kind: NodeKind::Identifier,
                    forward_parent: Some(3),
                    offset: 0,
                },
                Event::Token {
                    kind: TokenKind::Ident,
                    offset: 0,
                    len: 1,
                },
                Event::Finish,
                Event::Tombstone,
                Event::Finish,
            ],
        );
        assert!(tree.is_lossless(source));
        let children: Vec<_> = tree.root(source).children().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind(), NodeKind::Identifier);
        assert_eq!(children[0].text(), source);
    }

    #[test]
    fn empty_and_deep_arenas_are_iterative() {
        let empty = SyntaxTree::build("", &[], vec![]);
        assert!(empty.is_lossless(""));
        assert_eq!(empty.root("").span(), Span::new(0, 0));
        const DEPTH: usize = 16_384;
        let mut events = Vec::with_capacity(DEPTH * 2);
        events.push(start(NodeKind::Program, 0));
        events.extend((1..DEPTH).map(|_| start(NodeKind::Error, 0)));
        events.extend(std::iter::repeat_n(Event::Finish, DEPTH));
        let tree = SyntaxTree::build("", &[], events);
        assert_eq!(tree.root("").descendants().count(), DEPTH);
        assert_eq!(
            tree.root("")
                .descendants()
                .next_back()
                .unwrap()
                .ancestors()
                .count(),
            DEPTH
        );
        assert!(tree.clone().is_lossless(""));
    }
}

#[cfg(test)]
mod interpolation_tests {
    use super::*;

    #[test]
    fn explicit_offsets_preserve_interpolation_recovery_and_utf8() {
        let source = "\"${é + x}\"";
        let lexical = [
            Token::new(TokenKind::StringPart, 0, 1),
            Token::new(TokenKind::InterpolationStart, 1, 2),
            Token::new(TokenKind::Ident, 3, 2),
            Token::new(TokenKind::Whitespace, 5, 1),
            Token::new(TokenKind::Plus, 6, 1),
            Token::new(TokenKind::Whitespace, 7, 1),
            Token::new(TokenKind::Ident, 8, 1),
            Token::new(TokenKind::RBrace, 9, 1),
            Token::new(TokenKind::StringPart, 10, 1),
        ];
        let tree = SyntaxTree::build(
            source,
            &lexical,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Token {
                    kind: TokenKind::StringPart,
                    offset: 0,
                    len: 1,
                },
                Event::Start {
                    kind: NodeKind::StringInterpolation,
                    forward_parent: None,
                    offset: 1,
                },
                Event::Token {
                    kind: TokenKind::InterpolationStart,
                    offset: 1,
                    len: 2,
                },
                Event::Token {
                    kind: TokenKind::Ident,
                    offset: 8,
                    len: 1,
                },
                Event::Token {
                    kind: TokenKind::RBrace,
                    offset: 9,
                    len: 1,
                },
                Event::Finish,
                Event::Token {
                    kind: TokenKind::StringPart,
                    offset: 10,
                    len: 1,
                },
                Event::Finish,
            ],
        );
        assert!(tree.is_lossless(source));
        let names: Vec<_> = tree
            .identifier_tokens(source)
            .map(|token| token.text())
            .collect();
        assert_eq!(names, ["é", "x"]);
        let open = tree
            .tokens(source)
            .find(|token| token.kind() == TokenKind::InterpolationStart)
            .unwrap();
        let close = tree.closing_delimiter(source, open.id()).unwrap();
        assert_eq!(close.span(), Span::new(9, 10));
        assert_eq!(tree.opening_delimiter(source, close.id()), Some(open));
        assert_eq!(
            tree.root(source).children().next().unwrap().span(),
            Span::new(1, 10)
        );
    }

    #[test]
    fn generic_recovery_spans_do_not_change_nodes_or_source_extents() {
        let source = "x";
        let lexical = [Token::new(TokenKind::Ident, 0, 1)];
        let tree = SyntaxTree::build(
            source,
            &lexical,
            vec![
                Event::Start {
                    kind: NodeKind::Program,
                    forward_parent: None,
                    offset: 0,
                },
                Event::Start {
                    kind: NodeKind::Error,
                    forward_parent: None,
                    offset: 0,
                },
                Event::RecoverySpan(Span::new(0, 1)),
                Event::Finish,
                Event::Token {
                    kind: TokenKind::Ident,
                    offset: 0,
                    len: 1,
                },
                Event::RecoverySpan(Span::new(1, 1)),
                Event::Finish,
            ],
        );
        let root = tree.root(source);
        let missing = root.children().next().unwrap();
        assert_eq!(missing.span(), Span::new(0, 0));
        assert_eq!(missing.recovery_span(), Some(Span::new(0, 1)));
        assert_eq!(root.recovery_span(), Some(Span::new(1, 1)));
        assert_eq!(root.children().count(), 1);
        assert!(tree.is_lossless(source));
    }
}
