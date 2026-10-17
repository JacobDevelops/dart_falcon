use std::{borrow::Cow, fmt};

use super::*;
pub use crate::ast::{StringDelimiter, parse_string_delimiter};
use crate::token::TokenKind;

fn child<'a, T: AstNode<'a>>(node: SyntaxNode<'a>, index: usize) -> T {
    node.children()
        .filter_map(T::cast)
        .nth(index)
        .expect("required expression child")
}
fn name(node: SyntaxNode<'_>, index: usize) -> Identifier<'_> {
    node.children()
        .filter_map(Identifier::from_node)
        .nth(index)
        .expect("required expression identifier")
}
fn has(node: SyntaxNode<'_>, kind: TokenKind) -> bool {
    node.child_token(kind).is_some()
}
fn expr(node: SyntaxNode<'_>, index: usize) -> Expr<'_> {
    child(node, index)
}
fn types(node: SyntaxNode<'_>) -> AstChildren<'_, DartType<'_>> {
    TypeArgs::of(node)
}

#[derive(Clone, Copy)]
pub enum Expr<'a> {
    IntLit(IntLitExpr<'a>),
    DoubleLit(DoubleLitExpr<'a>),
    StringLit(StringLitNode<'a>),
    BoolLit(BoolLitExpr<'a>),
    NullLit(NullLitExpr<'a>),
    Ident(Identifier<'a>),
    Paren(ParenExpr<'a>),
    This(ThisExpr<'a>),
    Super(SuperExpr<'a>),
    Unary(UnaryExpr<'a>),
    PostfixIncDec(PostfixIncDecExpr<'a>),
    Binary(BinaryExpr<'a>),
    Assign(AssignExpr<'a>),
    Conditional(ConditionalExpr<'a>),
    Is(IsExpr<'a>),
    As(AsExpr<'a>),
    Field(FieldExpr<'a>),
    Index(IndexExpr<'a>),
    Call(CallExpr<'a>),
    Cascade(CascadeExpr<'a>),
    List(ListExpr<'a>),
    Map(MapExpr<'a>),
    Set(SetExpr<'a>),
    Record(RecordExpr<'a>),
    FuncExpr(FuncExpr<'a>),
    New(NewExpr<'a>),
    DotShorthand(DotShorthandExpr<'a>),
    Await(AwaitExpr<'a>),
    Throw(ThrowExpr<'a>),
    Switch(SwitchExpr<'a>),
    NullAssert(NullAssertExpr<'a>),
    SymbolLit(SymbolLitExpr<'a>),
    GenericInstantiation(GenericInstantiationExpr<'a>),
    Error(ErrorExpr<'a>),
}
impl<'a> AstNode<'a> for Expr<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        match node.kind() {
            NodeKind::IntLitExpr => Some(Self::IntLit(IntLitExpr(node))),
            NodeKind::DoubleLitExpr => Some(Self::DoubleLit(DoubleLitExpr(node))),
            NodeKind::StringLitNode | NodeKind::AdjacentStrings => {
                Some(Self::StringLit(StringLitNode(node)))
            }
            NodeKind::BoolLitExpr => Some(Self::BoolLit(BoolLitExpr(node))),
            NodeKind::NullLitExpr => Some(Self::NullLit(NullLitExpr(node))),
            NodeKind::Identifier => Some(Self::Ident(Identifier::from_node(node)?)),
            NodeKind::ParenExpr => Some(Self::Paren(ParenExpr(node))),
            NodeKind::ThisExpr => Some(Self::This(ThisExpr(node))),
            NodeKind::SuperExpr => Some(Self::Super(SuperExpr(node))),
            NodeKind::UnaryExpr => Some(Self::Unary(UnaryExpr(node))),
            NodeKind::PostfixIncDecExpr => Some(Self::PostfixIncDec(PostfixIncDecExpr(node))),
            NodeKind::BinaryExpr => Some(Self::Binary(BinaryExpr(node))),
            NodeKind::AssignExpr => Some(Self::Assign(AssignExpr(node))),
            NodeKind::ConditionalExpr => Some(Self::Conditional(ConditionalExpr(node))),
            NodeKind::IsExpr => Some(Self::Is(IsExpr(node))),
            NodeKind::AsExpr => Some(Self::As(AsExpr(node))),
            NodeKind::FieldExpr => Some(Self::Field(FieldExpr(node))),
            NodeKind::IndexExpr => Some(Self::Index(IndexExpr(node))),
            NodeKind::CallExpr => Some(Self::Call(CallExpr(node))),
            NodeKind::CascadeExpr => Some(Self::Cascade(CascadeExpr(node))),
            NodeKind::ListExpr => Some(Self::List(ListExpr(node))),
            NodeKind::MapExpr => Some(Self::Map(MapExpr(node))),
            NodeKind::SetExpr => Some(Self::Set(SetExpr(node))),
            NodeKind::RecordExpr => Some(Self::Record(RecordExpr(node))),
            NodeKind::FuncExpr => Some(Self::FuncExpr(FuncExpr(node))),
            NodeKind::NewExpr => Some(Self::New(NewExpr(node))),
            NodeKind::DotShorthandExpr => Some(Self::DotShorthand(DotShorthandExpr(node))),
            NodeKind::AwaitExpr => Some(Self::Await(AwaitExpr(node))),
            NodeKind::ThrowExpr => Some(Self::Throw(ThrowExpr(node))),
            NodeKind::SwitchExpr => Some(Self::Switch(SwitchExpr(node))),
            NodeKind::NullAssertExpr => Some(Self::NullAssert(NullAssertExpr(node))),
            NodeKind::SymbolLitExpr => Some(Self::SymbolLit(SymbolLitExpr(node))),
            NodeKind::GenericInstantiationExpr => {
                Some(Self::GenericInstantiation(GenericInstantiationExpr(node)))
            }
            NodeKind::ErrorExpr => Some(Self::Error(ErrorExpr(node))),
            _ => None,
        }
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::IntLit(node) => node.syntax(),
            Self::DoubleLit(node) => node.syntax(),
            Self::StringLit(node) => node.syntax(),
            Self::BoolLit(node) => node.syntax(),
            Self::NullLit(node) => node.syntax(),
            Self::Ident(node) => node.syntax(),
            Self::Paren(node) => node.syntax(),
            Self::This(node) => node.syntax(),
            Self::Super(node) => node.syntax(),
            Self::Unary(node) => node.syntax(),
            Self::PostfixIncDec(node) => node.syntax(),
            Self::Binary(node) => node.syntax(),
            Self::Assign(node) => node.syntax(),
            Self::Conditional(node) => node.syntax(),
            Self::Is(node) => node.syntax(),
            Self::As(node) => node.syntax(),
            Self::Field(node) => node.syntax(),
            Self::Index(node) => node.syntax(),
            Self::Call(node) => node.syntax(),
            Self::Cascade(node) => node.syntax(),
            Self::List(node) => node.syntax(),
            Self::Map(node) => node.syntax(),
            Self::Set(node) => node.syntax(),
            Self::Record(node) => node.syntax(),
            Self::FuncExpr(node) => node.syntax(),
            Self::New(node) => node.syntax(),
            Self::DotShorthand(node) => node.syntax(),
            Self::Await(node) => node.syntax(),
            Self::Throw(node) => node.syntax(),
            Self::Switch(node) => node.syntax(),
            Self::NullAssert(node) => node.syntax(),
            Self::SymbolLit(node) => node.syntax(),
            Self::GenericInstantiation(node) => node.syntax(),
            Self::Error(node) => node.syntax(),
        }
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> Expr<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        match self {
            Self::Ident(node) => node.span(),
            Self::Error(node) => node.span(),
            Self::StringLit(node) => node.span(),
            _ => self.syntax().span(),
        }
    }
}
impl fmt::Debug for Expr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntLit(node) => fmt::Debug::fmt(node, f),
            Self::DoubleLit(node) => fmt::Debug::fmt(node, f),
            Self::StringLit(node) => f.debug_tuple("StringLit").field(node).finish(),
            Self::BoolLit(node) => fmt::Debug::fmt(node, f),
            Self::NullLit(node) => fmt::Debug::fmt(node, f),
            Self::Ident(node) => f.debug_tuple("Ident").field(node).finish(),
            Self::Paren(node) => fmt::Debug::fmt(node, f),
            Self::This(node) => fmt::Debug::fmt(node, f),
            Self::Super(node) => fmt::Debug::fmt(node, f),
            Self::Unary(node) => fmt::Debug::fmt(node, f),
            Self::PostfixIncDec(node) => fmt::Debug::fmt(node, f),
            Self::Binary(node) => fmt::Debug::fmt(node, f),
            Self::Assign(node) => fmt::Debug::fmt(node, f),
            Self::Conditional(node) => fmt::Debug::fmt(node, f),
            Self::Is(node) => fmt::Debug::fmt(node, f),
            Self::As(node) => fmt::Debug::fmt(node, f),
            Self::Field(node) => fmt::Debug::fmt(node, f),
            Self::Index(node) => fmt::Debug::fmt(node, f),
            Self::Call(node) => fmt::Debug::fmt(node, f),
            Self::Cascade(node) => fmt::Debug::fmt(node, f),
            Self::List(node) => fmt::Debug::fmt(node, f),
            Self::Map(node) => fmt::Debug::fmt(node, f),
            Self::Set(node) => fmt::Debug::fmt(node, f),
            Self::Record(node) => fmt::Debug::fmt(node, f),
            Self::FuncExpr(node) => fmt::Debug::fmt(node, f),
            Self::New(node) => fmt::Debug::fmt(node, f),
            Self::DotShorthand(node) => fmt::Debug::fmt(node, f),
            Self::Await(node) => fmt::Debug::fmt(node, f),
            Self::Throw(node) => fmt::Debug::fmt(node, f),
            Self::Switch(node) => fmt::Debug::fmt(node, f),
            Self::NullAssert(node) => fmt::Debug::fmt(node, f),
            Self::SymbolLit(node) => fmt::Debug::fmt(node, f),
            Self::GenericInstantiation(node) => fmt::Debug::fmt(node, f),
            Self::Error(node) => fmt::Debug::fmt(node, f),
        }
    }
}

impl<'a> Expr<'a> {
    pub fn unparenthesized(self) -> Self {
        let mut node = self;
        while let Self::Paren(paren) = node {
            node = paren.expr();
        }
        node
    }
}

typed_node!(IntLitExpr, IntLitExpr);
debug_node!(IntLitExpr => "IntLit" { value, span });
typed_node!(DoubleLitExpr, DoubleLitExpr);
debug_node!(DoubleLitExpr => "DoubleLit" { value, span });
typed_node!(BoolLitExpr, BoolLitExpr);
debug_node!(BoolLitExpr => "BoolLit" { value, span });
typed_node!(NullLitExpr, NullLitExpr);
debug_node!(NullLitExpr => "NullLit" { span });
typed_node!(ParenExpr, ParenExpr);
debug_node!(ParenExpr => "Paren" { expr, span });
typed_node!(ThisExpr, ThisExpr);
debug_node!(ThisExpr => "This" { span });
typed_node!(SuperExpr, SuperExpr);
debug_node!(SuperExpr => "Super" { span });
typed_node!(UnaryExpr, UnaryExpr);
debug_node!(UnaryExpr => "Unary" { op, operand, span });
typed_node!(PostfixIncDecExpr, PostfixIncDecExpr);
debug_node!(PostfixIncDecExpr => "PostfixIncDec" { op, operand, span });
typed_node!(BinaryExpr, BinaryExpr);
debug_node!(BinaryExpr => "Binary" { op, op_span, left, right, span });
typed_node!(AssignExpr, AssignExpr);
debug_node!(AssignExpr => "Assign" { target, op, value, span });
typed_node!(ConditionalExpr, ConditionalExpr);
debug_node!(ConditionalExpr => "Conditional" { condition, then_expr, else_expr, span });
typed_node!(IsExpr, IsExpr);
debug_node!(IsExpr => "Is" { expr, dart_type, negated, span });
typed_node!(AsExpr, AsExpr);
debug_node!(AsExpr => "As" { expr, dart_type, span });
typed_node!(FieldExpr, FieldExpr);
debug_node!(FieldExpr => "Field" { object, field, is_null_safe, span });
typed_node!(IndexExpr, IndexExpr);
debug_node!(IndexExpr => "Index" { object, index, is_null_safe, span });
typed_node!(CallExpr, CallExpr);
debug_node!(CallExpr => "Call" { callee, type_args, args, span });
typed_node!(CascadeExpr, CascadeExpr);
debug_node!(CascadeExpr => "Cascade" { object, sections, is_null_aware, span });
typed_node!(ListExpr, ListExpr);
debug_node!(ListExpr => "List" { is_const, type_arg, elements, span });
typed_node!(MapExpr, MapExpr);
debug_node!(MapExpr => "Map" { is_const, type_args, elements, span });
typed_node!(SetExpr, SetExpr);
debug_node!(SetExpr => "Set" { is_const, type_arg, elements, span });
typed_node!(RecordExpr, RecordExpr);
debug_node!(RecordExpr => "Record" { is_const, fields, span });
typed_node!(FuncExpr, FuncExpr);
debug_node!(FuncExpr => "FuncExpr" { type_params, params, async_marker, body, span });
typed_node!(NewExpr, NewExpr);
debug_node!(NewExpr => "New" { is_const, dart_type, constructor_name, args, span });
typed_node!(DotShorthandExpr, DotShorthandExpr);
debug_node!(DotShorthandExpr => "DotShorthand" { is_const, name, span });
typed_node!(AwaitExpr, AwaitExpr);
debug_node!(AwaitExpr => "Await" { expr, span });
typed_node!(ThrowExpr, ThrowExpr);
debug_node!(ThrowExpr => "Throw" { expr, span });
typed_node!(SwitchExpr, SwitchExpr);
debug_node!(SwitchExpr => "Switch" { subject, arms, span });
typed_node!(NullAssertExpr, NullAssertExpr);
debug_node!(NullAssertExpr => "NullAssert" { operand, span });
typed_node!(SymbolLitExpr, SymbolLitExpr);
debug_node!(SymbolLitExpr => "SymbolLit" { raw, span });
typed_node!(GenericInstantiationExpr, GenericInstantiationExpr);
debug_node!(GenericInstantiationExpr => "GenericInstantiation" { target, type_args, span });
#[derive(Clone, Copy)]
pub struct ErrorExpr<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for ErrorExpr<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::ErrorExpr).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> ErrorExpr<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        let span = self.0.span();
        if span.start == span.end {
            self.0
                .tree()
                .token_at(self.0.source(), span.start)
                .map_or(span, |token| token.span())
        } else {
            span
        }
    }
}
debug_node!(ErrorExpr => "Error" { span });

impl<'a> IntLitExpr<'a> {
    pub fn value(self) -> &'a str {
        self.0.text()
    }
}
impl<'a> DoubleLitExpr<'a> {
    pub fn value(self) -> &'a str {
        self.0.text()
    }
}
impl<'a> BoolLitExpr<'a> {
    pub fn value(self) -> bool {
        has(self.0, TokenKind::True)
    }
}
impl<'a> ParenExpr<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}
impl<'a> UnaryExpr<'a> {
    pub fn operand(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn op(self) -> UnaryOp {
        match self.0.first_token().expect("unary operator").kind() {
            TokenKind::Minus => UnaryOp::Minus,
            TokenKind::Bang => UnaryOp::Bang,
            TokenKind::Tilde => UnaryOp::Tilde,
            TokenKind::PlusPlus => UnaryOp::PlusPlus,
            TokenKind::MinusMinus => UnaryOp::MinusMinus,
            _ => unreachable!("unary operator"),
        }
    }
}
impl<'a> PostfixIncDecExpr<'a> {
    pub fn operand(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn op(self) -> PostfixIncDec {
        if has(self.0, TokenKind::PlusPlus) {
            PostfixIncDec::Increment
        } else {
            PostfixIncDec::Decrement
        }
    }
}
impl<'a> BinaryExpr<'a> {
    pub fn left(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn right(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> AssignExpr<'a> {
    pub fn target(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> ConditionalExpr<'a> {
    pub fn condition(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn then_expr(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    pub fn else_expr(self) -> Expr<'a> {
        expr(self.0, 2)
    }
}
impl<'a> IsExpr<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn dart_type(self) -> DartType<'a> {
        child(self.0, 0)
    }
    pub fn negated(self) -> bool {
        has(self.0, TokenKind::Bang)
    }
}
impl<'a> AsExpr<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn dart_type(self) -> DartType<'a> {
        child(self.0, 0)
    }
}
impl<'a> FieldExpr<'a> {
    pub fn object(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn is_null_safe(self) -> bool {
        has(self.0, TokenKind::QmarkDot) || has(self.0, TokenKind::QmarkLBracket)
    }
    pub fn field(self) -> Identifier<'a> {
        self.0
            .children()
            .filter_map(Identifier::from_node)
            .next_back()
            .expect("field identifier")
    }
}
impl<'a> IndexExpr<'a> {
    pub fn object(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn index(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    pub fn is_null_safe(self) -> bool {
        has(self.0, TokenKind::QmarkDot) || has(self.0, TokenKind::QmarkLBracket)
    }
}
impl<'a> CallExpr<'a> {
    pub fn callee(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn args(self) -> ArgList<'a> {
        child(self.0, 0)
    }
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        types(self.0)
    }
}
impl<'a> CascadeExpr<'a> {
    pub fn object(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn sections(self) -> AstChildren<'a, CascadeSection<'a>> {
        AstChildren::new(self.0)
    }
    pub fn is_null_aware(self) -> bool {
        self.0
            .children_with_tokens()
            .filter_map(|element| match element {
                SyntaxElement::Token(token)
                    if matches!(token.kind(), TokenKind::DotDot | TokenKind::DotDotQmark) =>
                {
                    Some(token)
                }
                SyntaxElement::Node(node) if node.kind() == NodeKind::CascadeSection => {
                    node.first_token()
                }
                _ => None,
            })
            .next()
            .is_some_and(|token| token.kind() == TokenKind::DotDotQmark)
    }
}
impl<'a> AwaitExpr<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}
impl<'a> ThrowExpr<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}
impl<'a> SwitchExpr<'a> {
    pub fn subject(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn arms(self) -> AstChildren<'a, SwitchExprArm<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> NullAssertExpr<'a> {
    pub fn operand(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}
impl<'a> GenericInstantiationExpr<'a> {
    pub fn target(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        types(self.0)
    }
}
impl<'a> NewExpr<'a> {
    pub fn dart_type(self) -> DartType<'a> {
        child(self.0, 0)
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        self.0.children().filter_map(Identifier::from_node).next()
    }
    pub fn args(self) -> ArgList<'a> {
        child(self.0, 0)
    }
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
}
impl<'a> DotShorthandExpr<'a> {
    pub fn name(self) -> Identifier<'a> {
        name(self.0, 0)
    }
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
}
impl<'a> MapExpr<'a> {
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        types(self.0)
    }
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    pub fn elements(self) -> AstChildren<'a, MapElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> ListExpr<'a> {
    pub fn type_arg(self) -> Option<DartType<'a>> {
        types(self.0).next()
    }
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    pub fn elements(self) -> AstChildren<'a, CollectionElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> SetExpr<'a> {
    pub fn type_arg(self) -> Option<DartType<'a>> {
        types(self.0).next()
    }
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    pub fn elements(self) -> AstChildren<'a, CollectionElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> RecordExpr<'a> {
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    pub fn fields(self) -> AstChildren<'a, RecordField<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> FuncExpr<'a> {
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        AstChildren::new(self.0)
    }
    pub fn params(self) -> FormalParamList<'a> {
        child(self.0, 0)
    }
    pub fn body(self) -> FunctionBody<'a> {
        child(self.0, 0)
    }
    pub fn async_marker(self) -> AsyncMarker {
        if has(self.0, TokenKind::Async) {
            if has(self.0, TokenKind::Star) {
                AsyncMarker::AsyncStar
            } else {
                AsyncMarker::Async
            }
        } else if has(self.0, TokenKind::Sync) && has(self.0, TokenKind::Star) {
            AsyncMarker::SyncStar
        } else {
            AsyncMarker::Sync
        }
    }
}
impl<'a> SymbolLitExpr<'a> {
    pub fn raw(self) -> &'a str {
        self.0.text()
    }
}

fn operator(node: SyntaxNode<'_>) -> SyntaxToken<'_> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| !token.is_trivia())
        .expect("expression operator token")
}

fn assign_op(node: SyntaxNode<'_>) -> AssignOp {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find_map(|token| {
            Some(match token.kind() {
                TokenKind::Eq => AssignOp::Eq,
                TokenKind::PlusEq => AssignOp::PlusEq,
                TokenKind::MinusEq => AssignOp::MinusEq,
                TokenKind::StarEq => AssignOp::MulEq,
                TokenKind::SlashEq => AssignOp::DivEq,
                TokenKind::PercentEq => AssignOp::ModEq,
                TokenKind::TildeSlashEq => AssignOp::IntDivEq,
                TokenKind::AmpEq => AssignOp::AndEq,
                TokenKind::PipeEq => AssignOp::OrEq,
                TokenKind::CaretEq => AssignOp::XorEq,
                TokenKind::LtLtEq => AssignOp::ShlEq,
                TokenKind::GtGtEq => AssignOp::ShrEq,
                TokenKind::GtGtGtEq => AssignOp::UShrEq,
                TokenKind::QmarkQmarkEq => AssignOp::NullCoalesceEq,
                _ => return None,
            })
        })
        .expect("assignment operator")
}

impl BinaryExpr<'_> {
    pub fn op(self) -> BinaryOp {
        match operator(self.0).kind() {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Mod,
            TokenKind::TildeSlash => BinaryOp::IntDiv,
            TokenKind::EqEq => BinaryOp::EqEq,
            TokenKind::BangEq => BinaryOp::NotEq,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::LtEq => BinaryOp::LtEq,
            TokenKind::GtEq => BinaryOp::GtEq,
            TokenKind::AmpAmp => BinaryOp::And,
            TokenKind::PipePipe => BinaryOp::Or,
            TokenKind::Amp => BinaryOp::BitAnd,
            TokenKind::Pipe => BinaryOp::BitOr,
            TokenKind::Caret => BinaryOp::BitXor,
            TokenKind::LtLt => BinaryOp::Shl,
            TokenKind::GtGt => BinaryOp::Shr,
            TokenKind::GtGtGt => BinaryOp::UShr,
            TokenKind::QmarkQmark => BinaryOp::NullCoalesce,
            _ => unreachable!("binary operator"),
        }
    }
    pub fn op_span(self) -> Span {
        operator(self.0).span()
    }
}

impl AssignExpr<'_> {
    pub fn op(self) -> AssignOp {
        assign_op(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct StringLitNode<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for StringLitNode<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        matches!(
            node.kind(),
            NodeKind::StringLitNode | NodeKind::AdjacentStrings
        )
        .then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> StringLitNode<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        let span = self.0.span();
        if span.start == span.end {
            self.0
                .tree()
                .token_at(self.0.source(), span.start)
                .map_or(span, |token| token.span())
        } else {
            span
        }
    }
    fn text(self) -> &'a str {
        &self.0.source()[self.span().start..self.span().end]
    }
    pub fn is_adjacent(self) -> bool {
        self.0.kind() == NodeKind::AdjacentStrings
    }
    pub fn segment_spans(self) -> impl Iterator<Item = Span> + Clone {
        self.0
            .children()
            .filter(|node| node.kind() == NodeKind::StringLitNode)
            .map(|node| node.span())
    }
    pub fn raw(self) -> Cow<'a, str> {
        if !self.is_adjacent() {
            return Cow::Borrowed(self.text());
        }
        Cow::Owned(
            self.0
                .children()
                .filter_map(Self::cast)
                .map(|node| node.text())
                .collect(),
        )
    }
    pub fn segments(self) -> impl Iterator<Item = (StringDelimiter<'a>, Span)> + Clone {
        let single = (!self.is_adjacent()).then_some(self.0);
        single
            .into_iter()
            .chain(
                self.0
                    .children()
                    .filter(|node| node.kind() == NodeKind::StringLitNode),
            )
            .filter_map(Self::cast)
            .filter_map(|node| {
                parse_string_delimiter(node.text()).map(|delimiter| (delimiter, node.span()))
            })
    }
    pub fn delimiter(self) -> Option<StringDelimiter<'a>> {
        if self.is_adjacent() {
            None
        } else {
            parse_string_delimiter(self.text())
        }
    }
    pub fn has_raw_segment(self) -> bool {
        if self.is_adjacent() {
            self.0
                .children()
                .filter_map(Self::cast)
                .any(|node| node.text().starts_with('r'))
        } else {
            self.text().starts_with('r')
        }
    }
    pub fn value(self) -> Cow<'a, str> {
        if let Some(delimiter) = self.delimiter() {
            return Cow::Borrowed(delimiter.content);
        }
        if !self.is_adjacent() {
            return Cow::Borrowed(self.text().strip_prefix('r').unwrap_or(self.text()));
        }
        Cow::Owned(
            self.segments()
                .map(|(delimiter, _)| delimiter.content)
                .collect(),
        )
    }
    pub fn interpolations(self) -> impl Iterator<Item = StringInterpolation<'a>> + Clone {
        self.0
            .descendants()
            .filter_map(StringInterpolation::cast)
            .filter(move |node| {
                node.syntax()
                    .ancestors()
                    .skip(1)
                    .find(|parent| {
                        matches!(
                            parent.kind(),
                            NodeKind::StringLitNode | NodeKind::AdjacentStrings
                        )
                    })
                    .is_some_and(|parent| parent == self.0 || parent.parent() == Some(self.0))
            })
    }
}
impl fmt::Debug for StringLitNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StringLitNode")
            .field("raw", &self.raw())
            .field("is_adjacent", &self.is_adjacent())
            .field("segment_spans", &macros::DebugList(self.segment_spans()))
            .field("has_raw_segment", &self.has_raw_segment())
            .field("value", &self.value())
            .field("span", &self.span())
            .field("interpolations", &macros::DebugList(self.interpolations()))
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct StringInterpolation<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for StringInterpolation<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::StringInterpolation).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> StringInterpolation<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn span(self) -> Span {
        let first = self.0.first_token().expect("interpolation opener");
        let last = self.0.last_token().expect("interpolation end");
        Span::new(
            first.span().end,
            if first.kind() == TokenKind::InterpolationStart && last.kind() == TokenKind::RBrace {
                last.offset()
            } else {
                last.span().end
            },
        )
    }
}
debug_node!(StringInterpolation { expr, span });

typed_node!(CascadeSection, CascadeSection);
debug_node!(CascadeSection { ops, span });
impl<'a> CascadeSection<'a> {
    pub fn ops(self) -> AstChildren<'a, CascadeOp<'a>> {
        AstChildren::new(self.0)
    }
}

typed_node!(CascadeField, CascadeField);
debug_node!(CascadeField => "Field" { name, is_null_aware });
typed_node!(CascadeIndex, CascadeIndex);
debug_node!(CascadeIndex => "Index" { index, is_null_aware, span });
typed_node!(CascadeNullAssert, CascadeNullAssert);
debug_node!(CascadeNullAssert => "NullAssert" { span });
typed_node!(CascadeCall, CascadeCall);
debug_tuple!(CascadeCall => "Call" { name, type_args, args });
typed_node!(CascadeInvoke, CascadeInvoke);
debug_tuple!(CascadeInvoke => "Invoke" { type_args, args });
typed_node!(CascadeAssign, CascadeAssign);
debug_tuple!(CascadeAssign => "Assign" { target, op, value });
typed_node!(CascadeIndexAssign, CascadeIndexAssign);
debug_tuple!(CascadeIndexAssign => "IndexAssign" { target, op, value });

impl<'a> CascadeField<'a> {
    pub fn name(self) -> Identifier<'a> {
        name(self.0, 0)
    }
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::QmarkDot)
            || self.0.parent().is_some_and(|parent| {
                parent
                    .first_token()
                    .is_some_and(|token| token.kind() == TokenKind::DotDotQmark)
                    && parent.children().next() == Some(self.0)
            })
    }
}
impl<'a> CascadeIndex<'a> {
    pub fn index(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::QmarkLBracket)
            || self.0.parent().is_some_and(|parent| {
                parent
                    .first_token()
                    .is_some_and(|token| token.kind() == TokenKind::DotDotQmark)
                    && parent.children().next() == Some(self.0)
            })
    }
}
impl<'a> CascadeCall<'a> {
    pub fn name(self) -> Identifier<'a> {
        name(self.0, 0)
    }
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        types(self.0)
    }
    pub fn args(self) -> ArgList<'a> {
        child(self.0, 0)
    }
}
impl<'a> CascadeInvoke<'a> {
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        types(self.0)
    }
    pub fn args(self) -> ArgList<'a> {
        child(self.0, 0)
    }
}
impl<'a> CascadeAssign<'a> {
    pub fn target(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn op(self) -> AssignOp {
        assign_op(self.0)
    }
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> CascadeIndexAssign<'a> {
    pub fn target(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn op(self) -> AssignOp {
        assign_op(self.0)
    }
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}

typed_node!(NullAwareElement, NullAwareElement);
debug_node!(NullAwareElement => "NullAware" { expr, span });
typed_node!(SpreadElement, SpreadElement);
debug_node!(SpreadElement => "Spread" { expr, is_null_aware, span });
typed_node!(IfElement, IfElement);
debug_node!(IfElement => "If" { condition, then_elem, else_elem, span });
typed_node!(ForElement, ForElement);
debug_node!(ForElement => "For" { is_await, header, element, span });
typed_node!(MapEntry, MapEntry);
debug_node!(MapEntry {
    key,
    value,
    key_null_aware,
    value_null_aware,
    span
});
typed_node!(MapSpreadElement, MapSpreadElement);
debug_node!(MapSpreadElement => "Spread" { expr, is_null_aware, span });
typed_node!(MapIfElement, MapIfElement);
debug_node!(MapIfElement => "If" { condition, then_entry, else_entry, span });
typed_node!(MapForElement, MapForElement);
debug_node!(MapForElement => "For" { is_await, header, entry, span });

impl<'a> NullAwareElement<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}
impl<'a> SpreadElement<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::DotDotDotQmark)
    }
}
impl<'a> MapSpreadElement<'a> {
    pub fn expr(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::DotDotDotQmark)
    }
}
impl<'a> IfElement<'a> {
    pub fn condition(self) -> IfCondition<'a> {
        let node = self
            .0
            .children()
            .find(|node| node.kind() == NodeKind::ExprCondition)
            .expect("collection condition");
        child(node, 0)
    }
    pub fn then_elem(self) -> CollectionElement<'a> {
        child(self.0, 0)
    }
    pub fn else_elem(self) -> Option<CollectionElement<'a>> {
        AstChildren::new(self.0).nth(1)
    }
}
impl<'a> MapIfElement<'a> {
    pub fn condition(self) -> IfCondition<'a> {
        let node = self
            .0
            .children()
            .find(|node| node.kind() == NodeKind::ExprCondition)
            .expect("map condition");
        child(node, 0)
    }
    pub fn then_entry(self) -> MapElement<'a> {
        child(self.0, 0)
    }
    pub fn else_entry(self) -> Option<MapElement<'a>> {
        AstChildren::new(self.0).nth(1)
    }
}
impl<'a> ForElement<'a> {
    pub fn is_await(self) -> bool {
        has(self.0, TokenKind::Await)
    }
    pub fn header(self) -> ForHeader<'a> {
        child(self.0, 0)
    }
    pub fn element(self) -> CollectionElement<'a> {
        child(self.0, 0)
    }
}
impl<'a> MapForElement<'a> {
    pub fn is_await(self) -> bool {
        has(self.0, TokenKind::Await)
    }
    pub fn header(self) -> ForHeader<'a> {
        child(self.0, 0)
    }
    pub fn entry(self) -> MapElement<'a> {
        child(self.0, 0)
    }
}
impl<'a> MapEntry<'a> {
    pub fn key(self) -> Expr<'a> {
        expr(self.0, 0)
    }
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    pub fn key_null_aware(self) -> bool {
        self.0
            .first_token()
            .is_some_and(|token| token.kind() == TokenKind::Qmark)
    }
    pub fn value_null_aware(self) -> bool {
        self.0
            .children_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .skip_while(|token| token.kind() != TokenKind::Colon)
            .skip(1)
            .find(|token| !token.is_trivia())
            .is_some_and(|token| token.kind() == TokenKind::Qmark)
    }
}

#[derive(Clone, Copy)]
pub struct RecordField<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for RecordField<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::RecordField).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
debug_node!(RecordField { name, value, span });
impl<'a> RecordField<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        let mut span = self.0.span();
        if let Some(record) = self.0.parent()
            && record
                .children()
                .find(|node| node.kind() == NodeKind::RecordField)
                == Some(self.0)
        {
            span.start = record.span().start;
        }
        span
    }
    pub fn name(self) -> Option<Identifier<'a>> {
        has(self.0, TokenKind::Colon).then(|| name(self.0, 0))
    }
    pub fn value(self) -> Expr<'a> {
        self.0
            .children()
            .filter_map(Expr::cast)
            .next_back()
            .expect("record field value")
    }
}
typed_node!(ArgList, ArgList);
debug_node!(ArgList {
    positional,
    named,
    span
});
impl<'a> ArgList<'a> {
    pub fn positional(self) -> AstChildren<'a, Expr<'a>> {
        AstChildren::new(self.0)
    }
    pub fn named(self) -> AstChildren<'a, NamedArg<'a>> {
        AstChildren::new(self.0)
    }
}
typed_node!(NamedArg, NamedArg);
debug_node!(NamedArg { name, value, span });
impl<'a> NamedArg<'a> {
    pub fn name(self) -> Identifier<'a> {
        name(self.0, 0)
    }
    pub fn value(self) -> Expr<'a> {
        self.0
            .children()
            .filter_map(Expr::cast)
            .next_back()
            .expect("named argument value")
    }
}
typed_node!(SwitchExprArm, SwitchExprArm);
debug_node!(SwitchExprArm {
    pattern,
    guard,
    body,
    span
});
impl<'a> SwitchExprArm<'a> {
    pub fn pattern(self) -> Pattern<'a> {
        child(self.0, 0)
    }
    pub fn guard(self) -> Option<Expr<'a>> {
        self.0
            .children()
            .find(|node| node.kind() == NodeKind::ExprGuard)
            .map(|node| expr(node, 0))
    }
    pub fn body(self) -> Expr<'a> {
        expr(self.0, 0)
    }
}

macro_rules! expression_enum {
    ($name:ident { $($variant:ident($view:ident)),* } tuples { $($tuple:ident),* }) => {
        #[derive(Clone, Copy)]
        pub enum $name<'a> { $($variant($view<'a>)),* }
        impl<'a> AstNode<'a> for $name<'a> {
            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                $(if let Some(view) = $view::cast(node) { return Some(Self::$variant(view)); })*
                None
            }
            fn syntax(self) -> SyntaxNode<'a> { match self { $(Self::$variant(view) => view.syntax()),* } }
            fn span(self) -> Span { self.span() }
        }
        impl<'a> $name<'a> {
            pub fn cast(node: SyntaxNode<'a>) -> Option<Self> { <Self as AstNode>::cast(node) }
            pub fn syntax(self) -> SyntaxNode<'a> { <Self as AstNode>::syntax(self) }
            pub fn span(self) -> Span { match self { $(Self::$variant(view) => view.span()),* } }
        }
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $(if let Self::$tuple(view) = self {
                    return f.debug_tuple(stringify!($tuple)).field(view).finish();
                })*
                match self { $(Self::$variant(view) => fmt::Debug::fmt(view, f)),* }
            }
        }
    };
}

expression_enum!(CascadeOp {
    Field(CascadeField), Index(CascadeIndex), NullAssert(CascadeNullAssert),
    Call(CascadeCall), Invoke(CascadeInvoke), Assign(CascadeAssign), IndexAssign(CascadeIndexAssign)
} tuples {});
expression_enum!(CollectionElement {
    Expr(Expr), NullAware(NullAwareElement), Spread(SpreadElement), If(IfElement), For(ForElement)
} tuples { Expr });
expression_enum!(MapElement {
    Entry(MapEntry), Spread(MapSpreadElement), If(MapIfElement), For(MapForElement)
} tuples { Entry });
