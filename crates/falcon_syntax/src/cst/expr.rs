use std::{borrow::Cow, fmt};

use super::macros::DebugList;
use super::{
    AssignOp, AstChildren, AstNode, AsyncMarker, BinaryOp, DartType, ForHeader, FormalParamList,
    FunctionBody, Identifier, IfCondition, NodeKind, Pattern, PostfixIncDec, Span, StringDelimiter,
    SyntaxElement, SyntaxNode, SyntaxToken, TypeArgs, TypeParam, UnaryOp, parse_string_delimiter,
};
use crate::token::TokenKind;

fn has(node: SyntaxNode<'_>, kind: TokenKind) -> bool {
    node.child_token(kind).is_some()
}
fn expr(node: SyntaxNode<'_>, index: usize) -> Expr<'_> {
    node.children()
        .filter_map(Expr::cast)
        .nth(index)
        .expect("parser emits the required child")
}

fn collection_condition(node: SyntaxNode<'_>) -> IfCondition<'_> {
    node.children()
        .find(|node| node.kind() == NodeKind::ExprCondition)
        .expect("parser emits the collection condition")
        .required()
}

// One variant list defines the enum, casts, syntax mapping, spans, and debug
// dispatch. Expr also classifies every nonexpression kind explicitly: adding a
// NodeKind must never silently remove an expression from typed child iterators.
macro_rules! expression_enum {
    ($name:ident { $($variant:ident($view:ident => $($kind:ident)|+)),* }
        tuples { $($tuple:ident),* } non_expressions { $($other:pat_param)|+ }) => {
        expression_enum!(@define $name { $($variant($view)),* } tuples { $($tuple),* }
            cast |node| {
                match node.kind() {
                    $($(NodeKind::$kind)|+ => $view::cast(node).map(Self::$variant),)*
                    $($other)|+ => None,
                }
            }
        );
    };
    ($name:ident { $($variant:ident($view:ident)),* } tuples { $($tuple:ident),* }) => {
        expression_enum!(@define $name { $($variant($view)),* } tuples { $($tuple),* }
            cast |node| {
                $(if let Some(view) = $view::cast(node) { return Some(Self::$variant(view)); })*
                None
            }
        );
    };
    (@define $name:ident { $($variant:ident($view:ident)),* } tuples { $($tuple:ident),* }
        cast |$node:ident| $cast:block) => {
        #[derive(Clone, Copy)]
        pub enum $name<'a> { $($variant($view<'a>)),* }
        impl<'a> AstNode<'a> for $name<'a> {
            fn cast($node: SyntaxNode<'a>) -> Option<Self> { $cast }
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

expression_enum!(Expr {
    IntLit(IntLitExpr => IntLitExpr),
    DoubleLit(DoubleLitExpr => DoubleLitExpr),
    StringLit(StringLitNode => StringLitNode | AdjacentStrings),
    BoolLit(BoolLitExpr => BoolLitExpr),
    NullLit(NullLitExpr => NullLitExpr),
    Ident(Identifier => Identifier),
    Paren(ParenExpr => ParenExpr),
    This(ThisExpr => ThisExpr),
    Super(SuperExpr => SuperExpr),
    Unary(UnaryExpr => UnaryExpr),
    PostfixIncDec(PostfixIncDecExpr => PostfixIncDecExpr),
    Binary(BinaryExpr => BinaryExpr),
    Assign(AssignExpr => AssignExpr),
    Conditional(ConditionalExpr => ConditionalExpr),
    Is(IsExpr => IsExpr),
    As(AsExpr => AsExpr),
    Field(FieldExpr => FieldExpr),
    Index(IndexExpr => IndexExpr),
    Call(CallExpr => CallExpr),
    Cascade(CascadeExpr => CascadeExpr),
    List(ListExpr => ListExpr),
    Map(MapExpr => MapExpr),
    Set(SetExpr => SetExpr),
    Record(RecordExpr => RecordExpr),
    FuncExpr(FuncExpr => FuncExpr),
    New(NewExpr => NewExpr),
    DotShorthand(DotShorthandExpr => DotShorthandExpr),
    Await(AwaitExpr => AwaitExpr),
    Throw(ThrowExpr => ThrowExpr),
    Switch(SwitchExpr => SwitchExpr),
    NullAssert(NullAssertExpr => NullAssertExpr),
    SymbolLit(SymbolLitExpr => SymbolLitExpr),
    GenericInstantiation(GenericInstantiationExpr => GenericInstantiationExpr),
    Error(ErrorExpr => ErrorExpr)
} tuples { StringLit, Ident } non_expressions {
    NodeKind::Program
    | NodeKind::LibraryDirective
    | NodeKind::PartOfDirective
    | NodeKind::PartDirective
    | NodeKind::ImportDirective
    | NodeKind::ExportDirective
    | NodeKind::ConfigurableUri
    | NodeKind::Combinator
    | NodeKind::ClassDecl
    | NodeKind::PrimaryConstructor
    | NodeKind::PrimaryConstructorBody
    | NodeKind::ClassTypeAliasDecl
    | NodeKind::FieldDecl
    | NodeKind::VarDeclarator
    | NodeKind::ConstructorDecl
    | NodeKind::RedirectedConstructor
    | NodeKind::SuperCallInitializer
    | NodeKind::ThisCallInitializer
    | NodeKind::FieldInitializer
    | NodeKind::AssertInitializer
    | NodeKind::MethodDecl
    | NodeKind::GetterDecl
    | NodeKind::SetterDecl
    | NodeKind::OperatorDecl
    | NodeKind::MixinDecl
    | NodeKind::MixinClassDecl
    | NodeKind::EnumDecl
    | NodeKind::EnumVariant
    | NodeKind::ExtensionDecl
    | NodeKind::ExtensionTypeDecl
    | NodeKind::FunctionDecl
    | NodeKind::TopLevelVarDecl
    | NodeKind::TypeAliasDecl
    | NodeKind::FormalParamList
    | NodeKind::FormalParam(_)
    | NodeKind::ArrowBody
    | NodeKind::NativeBody
    | NodeKind::Annotation
    | NodeKind::ExtendsClause
    | NodeKind::WithClause
    | NodeKind::ImplementsClause
    | NodeKind::OnClause
    | NodeKind::SetterParameter
    | NodeKind::LegacyTypedef
    | NodeKind::NamedType
    | NodeKind::FunctionType
    | NodeKind::FunctionTypeParam(_)
    | NodeKind::RecordType
    | NodeKind::NamedRecordField
    | NodeKind::TypeParam
    | NodeKind::VoidType
    | NodeKind::DynamicType
    | NodeKind::TypeArgs
    | NodeKind::Block
    | NodeKind::IfStmt
    | NodeKind::CaseCondition
    | NodeKind::ForStmt
    | NodeKind::ForHeader
    | NodeKind::ForInInit
    | NodeKind::PatternForInInit
    | NodeKind::ForExprsInit
    | NodeKind::WhileStmt
    | NodeKind::DoWhileStmt
    | NodeKind::SwitchStmt
    | NodeKind::SwitchCase
    | NodeKind::CasePatternLabel
    | NodeKind::CaseExprLabel
    | NodeKind::DefaultLabel
    | NodeKind::TryCatchStmt
    | NodeKind::CatchClause
    | NodeKind::ReturnStmt
    | NodeKind::ThrowStmt
    | NodeKind::RethrowStmt
    | NodeKind::EmptyStmt
    | NodeKind::BreakStmt
    | NodeKind::ContinueStmt
    | NodeKind::LocalVarDecl
    | NodeKind::PatternDeclaration
    | NodeKind::PatternAssignStmt
    | NodeKind::LabeledStmt
    | NodeKind::LocalFuncDecl
    | NodeKind::AssertStmt
    | NodeKind::YieldStmt
    | NodeKind::ExprStmt
    | NodeKind::ErrorNode
    | NodeKind::ForCondition
    | NodeKind::ForUpdates
    | NodeKind::AwaitFor
    | NodeKind::StringInterpolation
    | NodeKind::CascadeSection
    | NodeKind::CascadeField
    | NodeKind::CascadeIndex
    | NodeKind::CascadeNullAssert
    | NodeKind::CascadeCall
    | NodeKind::CascadeInvoke
    | NodeKind::CascadeAssign
    | NodeKind::CascadeIndexAssign
    | NodeKind::NullAwareElement
    | NodeKind::SpreadElement
    | NodeKind::IfElement
    | NodeKind::ForElement
    | NodeKind::MapEntry
    | NodeKind::MapSpreadElement
    | NodeKind::MapIfElement
    | NodeKind::MapForElement
    | NodeKind::RecordField
    | NodeKind::ArgList
    | NodeKind::NamedArg
    | NodeKind::SwitchExprArm
    | NodeKind::ExprCondition
    | NodeKind::ExprGuard
    | NodeKind::WildcardPattern
    | NodeKind::VariablePattern
    | NodeKind::LiteralPattern
    | NodeKind::ConstPattern
    | NodeKind::ListPattern
    | NodeKind::RecordPattern
    | NodeKind::MapPattern
    | NodeKind::ObjectPattern
    | NodeKind::LogicalAndPattern
    | NodeKind::LogicalOrPattern
    | NodeKind::RelationalPattern
    | NodeKind::CastPattern
    | NodeKind::NullCheckPattern
    | NodeKind::NullAssertPattern
    | NodeKind::ParenPattern
    | NodeKind::ErrorPattern
    | NodeKind::RestPattern
    | NodeKind::RecordPatternField
    | NodeKind::MapPatternEntry
    | NodeKind::ObjectPatternField
    | NodeKind::ConstPatternExpr
    | NodeKind::Error
});

impl Expr<'_> {
    #[must_use]
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
    #[must_use]
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    #[must_use]
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    #[must_use]
    pub fn span(self) -> Span {
        let span = self.0.span();
        if span.start == span.end {
            self.0
                .tree()
                .token_at(self.0.source(), span.start)
                .map_or(span, SyntaxToken::span)
        } else {
            span
        }
    }
}
debug_node!(ErrorExpr => "Error" { span });

impl<'a> IntLitExpr<'a> {
    #[must_use]
    pub fn value(self) -> &'a str {
        self.0.text()
    }
}
impl<'a> DoubleLitExpr<'a> {
    #[must_use]
    pub fn value(self) -> &'a str {
        self.0.text()
    }
}
impl BoolLitExpr<'_> {
    #[must_use]
    pub fn value(self) -> bool {
        has(self.0, TokenKind::True)
    }
}
impl<'a> ParenExpr<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> UnaryExpr<'a> {
    #[must_use]
    pub fn operand(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    /// Returns the unary operator.
    ///
    /// # Panics
    /// Panics if the parser did not emit a unary operator token.
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
    #[must_use]
    pub fn operand(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn op(self) -> PostfixIncDec {
        if has(self.0, TokenKind::PlusPlus) {
            PostfixIncDec::Increment
        } else {
            PostfixIncDec::Decrement
        }
    }
}
impl<'a> BinaryExpr<'a> {
    #[must_use]
    pub fn left(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn right(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> AssignExpr<'a> {
    #[must_use]
    pub fn target(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> ConditionalExpr<'a> {
    #[must_use]
    pub fn condition(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn then_expr(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    #[must_use]
    pub fn else_expr(self) -> Expr<'a> {
        expr(self.0, 2)
    }
}
impl<'a> IsExpr<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn dart_type(self) -> DartType<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn negated(self) -> bool {
        has(self.0, TokenKind::Bang)
    }
}
impl<'a> AsExpr<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn dart_type(self) -> DartType<'a> {
        self.0.required()
    }
}
impl<'a> FieldExpr<'a> {
    #[must_use]
    pub fn object(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn is_null_safe(self) -> bool {
        has(self.0, TokenKind::QmarkDot) || has(self.0, TokenKind::QmarkLBracket)
    }
    #[must_use]
    /// Returns this field expression's identifier.
    ///
    /// # Panics
    /// Panics if the parser did not emit a field identifier.
    pub fn field(self) -> Identifier<'a> {
        AstChildren::<Identifier>::new(self.0)
            .next_back()
            .expect("field identifier")
    }
}
impl<'a> IndexExpr<'a> {
    #[must_use]
    pub fn object(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn index(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    #[must_use]
    pub fn is_null_safe(self) -> bool {
        has(self.0, TokenKind::QmarkDot) || has(self.0, TokenKind::QmarkLBracket)
    }
}
impl<'a> CallExpr<'a> {
    #[must_use]
    pub fn callee(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn args(self) -> ArgList<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
}
impl<'a> CascadeExpr<'a> {
    #[must_use]
    pub fn object(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn sections(self) -> AstChildren<'a, CascadeSection<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
    pub fn is_null_aware(self) -> bool {
        self.0
            .children_with_tokens()
            .find_map(|element| match element {
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
            .is_some_and(|token| token.kind() == TokenKind::DotDotQmark)
    }
}
impl<'a> AwaitExpr<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> ThrowExpr<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> SwitchExpr<'a> {
    #[must_use]
    pub fn subject(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn arms(self) -> AstChildren<'a, SwitchExprArm<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> NullAssertExpr<'a> {
    #[must_use]
    pub fn operand(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> GenericInstantiationExpr<'a> {
    #[must_use]
    pub fn target(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
}
impl<'a> NewExpr<'a> {
    #[must_use]
    pub fn dart_type(self) -> DartType<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>()
    }
    #[must_use]
    pub fn args(self) -> ArgList<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
}
impl<'a> DotShorthandExpr<'a> {
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
}
impl<'a> MapExpr<'a> {
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    #[must_use]
    pub fn elements(self) -> AstChildren<'a, MapElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> ListExpr<'a> {
    #[must_use]
    pub fn type_arg(self) -> Option<DartType<'a>> {
        TypeArgs::of(self.0).next()
    }
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    #[must_use]
    pub fn elements(self) -> AstChildren<'a, CollectionElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> SetExpr<'a> {
    #[must_use]
    pub fn type_arg(self) -> Option<DartType<'a>> {
        TypeArgs::of(self.0).next()
    }
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    #[must_use]
    pub fn elements(self) -> AstChildren<'a, CollectionElement<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> RecordExpr<'a> {
    #[must_use]
    pub fn is_const(self) -> bool {
        has(self.0, TokenKind::Const)
    }
    #[must_use]
    pub fn fields(self) -> AstChildren<'a, RecordField<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> FuncExpr<'a> {
    #[must_use]
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
    pub fn params(self) -> FormalParamList<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn body(self) -> FunctionBody<'a> {
        self.0.required()
    }
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn op_span(self) -> Span {
        operator(self.0).span()
    }
}

impl AssignExpr<'_> {
    #[must_use]
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
    #[must_use]
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    #[must_use]
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    #[must_use]
    pub fn span(self) -> Span {
        let span = self.0.span();
        if span.start == span.end {
            self.0
                .tree()
                .token_at(self.0.source(), span.start)
                .map_or(span, SyntaxToken::span)
        } else {
            span
        }
    }
    fn text(self) -> &'a str {
        &self.0.source()[self.span().start..self.span().end]
    }
    #[must_use]
    pub fn is_adjacent(self) -> bool {
        self.0.kind() == NodeKind::AdjacentStrings
    }
    pub fn segment_spans(self) -> impl Iterator<Item = Span> + Clone {
        self.0
            .children()
            .filter(|node| node.kind() == NodeKind::StringLitNode)
            .map(SyntaxNode::span)
    }
    pub fn raw(self) -> Cow<'a, str> {
        if !self.is_adjacent() {
            return Cow::Borrowed(self.text());
        }
        Cow::Owned(
            self.0
                .children()
                .filter_map(Self::cast)
                .map(Self::text)
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
    #[must_use]
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
    #[must_use]
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
            .field("segment_spans", &DebugList(self.segment_spans()))
            .field("has_raw_segment", &self.has_raw_segment())
            .field("value", &self.value())
            .field("span", &self.span())
            .field("interpolations", &DebugList(self.interpolations()))
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
    #[must_use]
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    #[must_use]
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    /// Returns the source span covered by this interpolation.
    ///
    /// # Panics
    /// Panics if the parser did not emit the interpolation opener and end tokens.
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
    #[must_use]
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
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    #[must_use]
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
    #[must_use]
    pub fn index(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
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
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    #[must_use]
    pub fn args(self) -> ArgList<'a> {
        self.0.required()
    }
}
impl<'a> CascadeInvoke<'a> {
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    #[must_use]
    pub fn args(self) -> ArgList<'a> {
        self.0.required()
    }
}
impl<'a> CascadeAssign<'a> {
    #[must_use]
    pub fn target(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn op(self) -> AssignOp {
        assign_op(self.0)
    }
    #[must_use]
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
}
impl<'a> CascadeIndexAssign<'a> {
    #[must_use]
    pub fn target(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn op(self) -> AssignOp {
        assign_op(self.0)
    }
    #[must_use]
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
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> SpreadElement<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::DotDotDotQmark)
    }
}
impl<'a> MapSpreadElement<'a> {
    #[must_use]
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn is_null_aware(self) -> bool {
        has(self.0, TokenKind::DotDotDotQmark)
    }
}
impl<'a> IfElement<'a> {
    #[must_use]
    pub fn condition(self) -> IfCondition<'a> {
        collection_condition(self.0)
    }
    #[must_use]
    pub fn then_elem(self) -> CollectionElement<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn else_elem(self) -> Option<CollectionElement<'a>> {
        AstChildren::new(self.0).nth(1)
    }
}
impl<'a> MapIfElement<'a> {
    #[must_use]
    pub fn condition(self) -> IfCondition<'a> {
        collection_condition(self.0)
    }
    #[must_use]
    pub fn then_entry(self) -> MapElement<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn else_entry(self) -> Option<MapElement<'a>> {
        AstChildren::new(self.0).nth(1)
    }
}
impl<'a> ForElement<'a> {
    #[must_use]
    pub fn is_await(self) -> bool {
        has(self.0, TokenKind::Await)
    }
    #[must_use]
    pub fn header(self) -> ForHeader<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn element(self) -> CollectionElement<'a> {
        self.0.required()
    }
}
impl<'a> MapForElement<'a> {
    #[must_use]
    pub fn is_await(self) -> bool {
        has(self.0, TokenKind::Await)
    }
    #[must_use]
    pub fn header(self) -> ForHeader<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn entry(self) -> MapElement<'a> {
        self.0.required()
    }
}
impl<'a> MapEntry<'a> {
    #[must_use]
    pub fn key(self) -> Expr<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn value(self) -> Expr<'a> {
        expr(self.0, 1)
    }
    #[must_use]
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
    #[must_use]
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    #[must_use]
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    #[must_use]
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
    #[must_use]
    pub fn name(self) -> Option<Identifier<'a>> {
        has(self.0, TokenKind::Colon).then(|| self.0.required())
    }
    /// Returns this record field's value expression.
    ///
    /// # Panics
    /// Panics if the parser did not emit a value expression.
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
    #[must_use]
    pub fn positional(self) -> AstChildren<'a, Expr<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
    pub fn named(self) -> AstChildren<'a, NamedArg<'a>> {
        AstChildren::new(self.0)
    }
}
typed_node!(NamedArg, NamedArg);
debug_node!(NamedArg { name, value, span });
impl<'a> NamedArg<'a> {
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    /// Returns the named argument's value expression.
    ///
    /// # Panics
    /// Panics if the parser did not emit a value expression.
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
    #[must_use]
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn guard(self) -> Option<Expr<'a>> {
        self.0
            .children()
            .find(|node| node.kind() == NodeKind::ExprGuard)
            .map(|node| expr(node, 0))
    }
    #[must_use]
    pub fn body(self) -> Expr<'a> {
        self.0.required()
    }
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

/// A collection or map element with its original syntax identity.
pub trait Element<'a>: AstNode<'a> {
    const IS_MAP: bool;

    fn kind(self) -> ElementKind<'a, Self>;

    fn view(self) -> ElementView<'a>;

    fn is_await(self) -> bool {
        let syntax = self.syntax();
        matches!(
            syntax.kind(),
            NodeKind::ForElement | NodeKind::MapForElement
        ) && has(syntax, TokenKind::Await)
    }

    fn spread_is_null_aware(self) -> bool {
        has(self.syntax(), TokenKind::DotDotDotQmark)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ElementView<'a> {
    Collection(CollectionElement<'a>),
    Map(MapElement<'a>),
}

#[derive(Debug, Clone, Copy)]
pub enum ElementKind<'a, E> {
    Expression(Expr<'a>),
    NullAware(NullAwareElement<'a>),
    Entry(MapEntry<'a>),
    Spread {
        expression: Expr<'a>,
    },
    If {
        condition: IfCondition<'a>,
        then_element: E,
        else_element: Option<E>,
    },
    For {
        header: ForHeader<'a>,
        body: E,
    },
}

impl<'a> Element<'a> for CollectionElement<'a> {
    const IS_MAP: bool = false;

    fn view(self) -> ElementView<'a> {
        ElementView::Collection(self)
    }

    fn kind(self) -> ElementKind<'a, Self> {
        match self {
            Self::Expr(expression) => ElementKind::Expression(expression),
            Self::NullAware(element) => ElementKind::NullAware(element),
            Self::Spread(element) => ElementKind::Spread {
                expression: element.expr(),
            },
            Self::If(element) => ElementKind::If {
                condition: element.condition(),
                then_element: element.then_elem(),
                else_element: element.else_elem(),
            },
            Self::For(element) => ElementKind::For {
                header: element.header(),
                body: element.element(),
            },
        }
    }
}

impl<'a> Element<'a> for MapElement<'a> {
    const IS_MAP: bool = true;

    fn view(self) -> ElementView<'a> {
        ElementView::Map(self)
    }

    fn kind(self) -> ElementKind<'a, Self> {
        match self {
            Self::Entry(entry) => ElementKind::Entry(entry),
            Self::Spread(element) => ElementKind::Spread {
                expression: element.expr(),
            },
            Self::If(element) => ElementKind::If {
                condition: element.condition(),
                then_element: element.then_entry(),
                else_element: element.else_entry(),
            },
            Self::For(element) => ElementKind::For {
                header: element.header(),
                body: element.entry(),
            },
        }
    }
}
