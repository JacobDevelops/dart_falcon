use std::fmt;

use super::{
    AstChildren, AstNode, DartType, Expr, Identifier, NodeKind, RelationalPatternOp, Span,
    StringLitNode, SyntaxNode, SyntaxNodeChildren, TypeArgs,
};
use crate::token::TokenKind;

macro_rules! pattern_node {
    (ParenPattern) => {
        #[derive(Clone, Copy)]
        pub struct ParenPattern<'a>(pub(crate) SyntaxNode<'a>);
        impl<'a> AstNode<'a> for ParenPattern<'a> {
            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                (node.kind() == NodeKind::ParenPattern).then_some(Self(node))
            }
            fn syntax(self) -> SyntaxNode<'a> {
                self.0
            }
            fn span(self) -> Span {
                self.span()
            }
        }
        impl<'a> ParenPattern<'a> {
            pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                <Self as AstNode>::cast(node)
            }
            pub fn syntax(self) -> SyntaxNode<'a> {
                self.0
            }
            pub fn span(self) -> Span {
                let span = self.0.span();
                if self.0.child_token(TokenKind::RParen).is_none() {
                    let end = self
                        .0
                        .recovery_span()
                        .expect("missing pattern close preserves its lexical span")
                        .end;
                    Span::new(span.start, end)
                } else {
                    span
                }
            }
        }
    };
    ($view:ident) => {
        typed_node!($view, $view);
    };
}

macro_rules! patterns {
    ($($variant:ident($view:ident)),* $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum Pattern<'a> { $($variant($view<'a>)),* }
        impl<'a> AstNode<'a> for Pattern<'a> {
            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                match node.kind() { $(NodeKind::$view => Some(Self::$variant($view(node))),)* _ => None }
            }
            fn syntax(self) -> SyntaxNode<'a> { match self { $(Self::$variant(view) => view.syntax()),* } }
            fn span(self) -> Span { self.span() }
        }
        impl<'a> Pattern<'a> {
            pub fn cast(node: SyntaxNode<'a>) -> Option<Self> { <Self as AstNode>::cast(node) }
            pub fn syntax(self) -> SyntaxNode<'a> { <Self as AstNode>::syntax(self) }
            pub fn span(self) -> Span { match self { $(Self::$variant(view) => view.span()),* } }
        }
        impl fmt::Debug for Pattern<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant(view) => pattern_debug(stringify!($variant), view, f)),*
                }
            }
        }
        $(pattern_node!($view);)*
    };
}
patterns! {
    Wildcard(WildcardPattern), Variable(VariablePattern), Literal(LiteralPattern), Const(ConstPattern),
    List(ListPattern), Record(RecordPattern), Map(MapPattern), Object(ObjectPattern),
    LogicalAnd(LogicalAndPattern), LogicalOr(LogicalOrPattern), Relational(RelationalPattern),
    Cast(CastPattern), NullCheck(NullCheckPattern), NullAssert(NullAssertPattern),
    ParenPattern(ParenPattern), Error(ErrorPattern),
}

fn pattern_debug(name: &str, value: &impl fmt::Debug, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if matches!(
        name,
        "Literal" | "Const" | "List" | "Record" | "Map" | "Object"
    ) {
        f.debug_tuple(name).field(value).finish()
    } else {
        value.fmt(f)
    }
}

impl<'a> WildcardPattern<'a> {
    pub fn type_(self) -> Option<DartType<'a>> {
        self.0.child()
    }
}
impl<'a> VariablePattern<'a> {
    pub fn type_(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
}
debug_node!(WildcardPattern => "Wildcard" { type_, span });
debug_node!(VariablePattern => "Variable" { type_, name, span });
debug_node!(ErrorPattern => "Error" { span });

macro_rules! binary_pattern {
    ($view:ident, $name:literal) => {
        impl<'a> $view<'a> {
            pub fn left(self) -> Pattern<'a> { self.0.required() }
            pub fn right(self) -> Pattern<'a> { self.0.children().filter_map(Pattern::cast).nth(1).expect("parser emits the required child") }
        }
        debug_node!($view => $name { left, right, span });
    };
}
binary_pattern!(LogicalAndPattern, "LogicalAnd");
binary_pattern!(LogicalOrPattern, "LogicalOr");
macro_rules! inner_pattern {
    ($view:ident, $name:literal) => {
        impl<'a> $view<'a> { pub fn inner(self) -> Pattern<'a> { self.0.required() } }
        debug_node!($view => $name { inner, span });
    };
}
inner_pattern!(NullCheckPattern, "NullCheck");
inner_pattern!(NullAssertPattern, "NullAssert");
inner_pattern!(ParenPattern, "ParenPattern");
impl<'a> CastPattern<'a> {
    pub fn inner(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn cast_type(self) -> DartType<'a> {
        self.0.required()
    }
}
debug_node!(CastPattern => "Cast" { inner, cast_type, span });
impl<'a> RelationalPattern<'a> {
    pub fn value(self) -> Expr<'a> {
        self.0.required()
    }
    pub fn op(self) -> RelationalPatternOp {
        match self
            .0
            .first_token()
            .expect("relational pattern has an operator")
            .kind()
        {
            TokenKind::EqEq => RelationalPatternOp::Eq,
            TokenKind::BangEq => RelationalPatternOp::NotEq,
            TokenKind::Lt => RelationalPatternOp::Lt,
            TokenKind::Gt => RelationalPatternOp::Gt,
            TokenKind::LtEq => RelationalPatternOp::LtEq,
            TokenKind::GtEq => RelationalPatternOp::GtEq,
            _ => unreachable!("relational pattern operator"),
        }
    }
}
debug_node!(RelationalPattern => "Relational" { op, value, span });

#[derive(Clone, Copy, Debug)]
pub enum LiteralPatternValue<'a> {
    Null,
    Bool(bool),
    Int(&'a str),
    Double(&'a str),
    String(StringLitNode<'a>),
    NegInt(&'a str),
    NegDouble(&'a str),
}
impl<'a> LiteralPattern<'a> {
    pub fn value(self) -> LiteralPatternValue<'a> {
        if let Some(string) = self.0.child::<StringLitNode<'a>>() {
            return LiteralPatternValue::String(string);
        }
        let mut tokens = self.0.significant_tokens();
        let first = tokens.next().expect("literal pattern has a token");
        match first.kind() {
            TokenKind::Null => LiteralPatternValue::Null,
            TokenKind::True => LiteralPatternValue::Bool(true),
            TokenKind::False => LiteralPatternValue::Bool(false),
            TokenKind::IntLit => LiteralPatternValue::Int(first.text()),
            TokenKind::DoubleLit => LiteralPatternValue::Double(first.text()),
            TokenKind::Minus => {
                let number = tokens.next().expect("negative literal has a number");
                if number.kind() == TokenKind::IntLit {
                    LiteralPatternValue::NegInt(number.text())
                } else {
                    LiteralPatternValue::NegDouble(number.text())
                }
            }
            _ => unreachable!("literal pattern token"),
        }
    }
}
debug_node!(LiteralPattern { value, span });

#[derive(Clone)]
pub struct PatternNames<'a> {
    nodes: SyntaxNodeChildren<'a>,
    dynamic: Option<Identifier<'a>>,
}
impl<'a> PatternNames<'a> {
    pub fn len(&self) -> usize {
        self.clone().count()
    }
    pub fn is_empty(&self) -> bool {
        self.first().is_none()
    }
    pub fn first(&self) -> Option<Identifier<'a>> {
        self.clone().next()
    }
    pub fn last(&self) -> Option<Identifier<'a>> {
        Iterator::last(self.clone())
    }
}
impl<'a> Iterator for PatternNames<'a> {
    type Item = Identifier<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.dynamic
            .take()
            .or_else(|| self.nodes.find_map(Identifier::from_node))
    }
}
impl fmt::Debug for PatternNames<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
#[derive(Clone, Debug)]
pub enum ConstPatternValue<'a> {
    Name(PatternNames<'a>),
    Expr(Expr<'a>),
}
impl<'a> ConstPattern<'a> {
    pub fn value(self) -> ConstPatternValue<'a> {
        if let Some(value) = self
            .0
            .children()
            .find(|node| node.kind() == NodeKind::ConstPatternExpr)
        {
            return ConstPatternValue::Expr(value.required());
        }
        let names = self
            .0
            .children()
            .find(|node| matches!(node.kind(), NodeKind::NamedType | NodeKind::DynamicType));
        let node = names.unwrap_or(self.0);
        ConstPatternValue::Name(PatternNames {
            nodes: node.children(),
            dynamic: (node.kind() == NodeKind::DynamicType)
                .then(|| Identifier::synthetic(node, "dynamic", node.span())),
        })
    }
}
debug_node!(ConstPattern { value, span });

#[derive(Clone, Copy)]
pub enum ListPatternElement<'a> {
    Pattern(Pattern<'a>),
    Rest(RestPattern<'a>),
}
impl<'a> AstNode<'a> for ListPatternElement<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        RestPattern::cast(node)
            .map(Self::Rest)
            .or_else(|| Pattern::cast(node).map(Self::Pattern))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Pattern(p) => p.syntax(),
            Self::Rest(p) => p.syntax(),
        }
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> ListPatternElement<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        match self {
            Self::Pattern(p) => p.span(),
            Self::Rest(p) => p.span(),
        }
    }
}
impl fmt::Debug for ListPatternElement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pattern(pattern) => f.debug_tuple("Pattern").field(pattern).finish(),
            Self::Rest(rest) => rest.fmt(f),
        }
    }
}
typed_node!(RestPattern, RestPattern);
impl<'a> RestPattern<'a> {
    pub fn pattern(self) -> Option<Pattern<'a>> {
        self.0.child()
    }
}
debug_tuple!(RestPattern => "Rest" { pattern, span });
impl<'a> ListPattern<'a> {
    pub fn type_arg(self) -> Option<DartType<'a>> {
        TypeArgs::of(self.0).next()
    }
    pub fn elements(self) -> AstChildren<'a, ListPatternElement<'a>> {
        AstChildren::new(self.0)
    }
}
debug_node!(ListPattern {
    type_arg,
    elements,
    span
});

impl<'a> RecordPattern<'a> {
    pub fn fields(self) -> AstChildren<'a, RecordPatternField<'a>> {
        AstChildren::new(self.0)
    }
}
debug_node!(RecordPattern { fields, span });
#[derive(Clone, Copy)]
pub struct RecordPatternField<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for RecordPatternField<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::RecordPatternField).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> RecordPatternField<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn name(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>().or_else(|| {
            (self
                .0
                .first_token()
                .is_some_and(|token| token.kind() == TokenKind::Colon))
            .then(|| binding_name(self.pattern()))
            .flatten()
        })
    }
    pub fn span(self) -> Span {
        let span = self.0.span();
        let parent = self.0.parent().expect("record field has a record parent");
        let first = parent
            .children()
            .find(|node| node.kind() == NodeKind::RecordPatternField);
        if first == Some(self.0)
            && self.name().is_none()
            && self
                .0
                .first_token()
                .is_none_or(|token| token.kind() != TokenKind::Colon)
        {
            let end = if self.0.first_token().is_none() {
                parent
                    .first_token()
                    .expect("record has an opening paren")
                    .span()
                    .end
            } else {
                span.end
            };
            Span::new(parent.span().start, end)
        } else {
            span
        }
    }
    pub fn source_name(self) -> Option<Identifier<'a>> {
        self.name()
            .filter(|name| name.span().end <= self.pattern().span().start)
    }
}
debug_node!(RecordPatternField {
    name,
    pattern,
    span
});

impl<'a> MapPattern<'a> {
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    pub fn entries(self) -> AstChildren<'a, MapPatternEntry<'a>> {
        AstChildren::new(self.0)
    }
    pub fn has_rest(self) -> bool {
        self.0.child_token(TokenKind::DotDotDot).is_some()
    }
}
debug_node!(MapPattern {
    type_args,
    entries,
    has_rest,
    span
});
typed_node!(MapPatternEntry, MapPatternEntry);
impl<'a> MapPatternEntry<'a> {
    pub fn key(self) -> Expr<'a> {
        self.0.required()
    }
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
}
debug_node!(MapPatternEntry { key, pattern, span });
impl<'a> ObjectPattern<'a> {
    pub fn type_(self) -> DartType<'a> {
        self.0.required()
    }
    pub fn fields(self) -> AstChildren<'a, ObjectPatternField<'a>> {
        AstChildren::new(self.0)
    }
}
debug_node!(ObjectPattern {
    type_,
    fields,
    span
});
typed_node!(ObjectPatternField, ObjectPatternField);
impl<'a> ObjectPatternField<'a> {
    pub fn name(self) -> Identifier<'a> {
        let shorthand = self
            .0
            .first_token()
            .is_some_and(|token| token.kind() == TokenKind::Colon);
        self.0
            .child()
            .or_else(|| {
                shorthand
                    .then(|| self.pattern().and_then(binding_name))
                    .flatten()
            })
            .unwrap_or_else(|| {
                let name = if shorthand { "<shorthand>" } else { "<field>" };
                let span = self
                    .0
                    .recovery_span()
                    .expect("object field recovery preserves the lexical name span");
                Identifier::synthetic(self.0, name, span)
            })
    }
    pub fn pattern(self) -> Option<Pattern<'a>> {
        self.0.child()
    }
    pub fn source_name(self) -> Option<Identifier<'a>> {
        let name = self.name();
        let span = name.span();
        (self.span().start <= span.start
            && span.start < span.end
            && span.end <= self.span().end
            && self
                .pattern()
                .is_none_or(|pattern| span.end <= pattern.span().start))
        .then_some(name)
    }
}
debug_node!(ObjectPatternField {
    name,
    pattern,
    span
});

fn binding_name(pattern: Pattern<'_>) -> Option<Identifier<'_>> {
    match pattern {
        Pattern::Variable(pattern) => Some(pattern.name()),
        Pattern::Cast(pattern) => binding_name(pattern.inner()),
        Pattern::NullCheck(pattern) => binding_name(pattern.inner()),
        Pattern::NullAssert(pattern) => binding_name(pattern.inner()),
        _ => None,
    }
}
