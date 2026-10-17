use std::fmt;

use super::*;
use crate::token::TokenKind;

fn keyword(node: SyntaxNode<'_>) -> VarKeyword {
    match keyword_token(node).map(|token| token.kind()) {
        Some(TokenKind::Const) => VarKeyword::Const,
        Some(TokenKind::Final) => VarKeyword::Final,
        _ => VarKeyword::Var,
    }
}
fn keyword_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| {
            matches!(
                token.kind(),
                TokenKind::Var | TokenKind::Final | TokenKind::Const
            )
        })
}

macro_rules! statement_enum {
    ($($variant:ident($ty:ident)),* $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum Stmt<'a> { $($variant($ty<'a>)),* }
        impl<'a> AstNode<'a> for Stmt<'a> {
            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                $(if let Some(value) = $ty::cast(node) { return Some(Self::$variant(value)); })*
                None
            }
            fn syntax(self) -> SyntaxNode<'a> { match self { $(Self::$variant(value) => value.syntax()),* } }
            fn span(self) -> Span { match self { $(Self::$variant(value) => value.span()),* } }
        }
        impl<'a> Stmt<'a> {
            pub fn cast(node: SyntaxNode<'a>) -> Option<Self> { <Self as AstNode>::cast(node) }
            pub fn syntax(self) -> SyntaxNode<'a> { <Self as AstNode>::syntax(self) }
            pub fn span(self) -> Span { <Self as AstNode>::span(self) }
        }
        impl fmt::Debug for Stmt<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if let Self::Rethrow(value) = self {
                    return f.debug_tuple("Rethrow").field(&value.span()).finish();
                }
                if let Self::Empty(value) = self {
                    return f.debug_tuple("Empty").field(&value.span()).finish();
                }
                match self {
                    $(Self::$variant(value) => f.debug_tuple(stringify!($variant)).field(value).finish()),*
                }
            }
        }
    };
}
statement_enum! {
    Block(Block), If(IfStmt), For(ForStmt), While(WhileStmt), DoWhile(DoWhileStmt),
    Switch(SwitchStmt), TryCatch(TryCatchStmt), Return(ReturnStmt), Throw(ThrowStmt),
    Break(BreakStmt), Continue(ContinueStmt), LocalVar(LocalVarDecl),
    PatternDecl(PatternDeclaration), PatternAssign(PatternAssignStmt), Labeled(LabeledStmt),
    LocalFunc(LocalFuncDecl), Assert(AssertStmt), Yield(YieldStmt), Expr(ExprStmt), Error(ErrorNode),
    Rethrow(RethrowStmt), Empty(EmptyStmt),
}

macro_rules! nodes {
    ($($ty:ident { $($field:ident),* }),* $(,)?) => { $(typed_node!($ty, $ty); debug_node!($ty { $($field),* });)* };
}
nodes! {
    IfStmt { condition, then_branch, else_branch, span },
    ForStmt { is_await, header, body, span },
    ForHeader { init, condition, updates },
    WhileStmt { condition, body, span },
    DoWhileStmt { body, condition, span },
    SwitchStmt { subject, cases, span },
    TryCatchStmt { body, catches, finally, span },
    CatchClause { exception_type, exception_var, stack_trace_var, body, span },
    ReturnStmt { value, span },
    ThrowStmt { value, span },
    BreakStmt { label, span },
    ContinueStmt { label, span },
    PatternDeclaration { annotations, keyword, keyword_span, pattern, init, span },
    PatternAssignStmt { pattern, value, span },
    LabeledStmt { label, stmt, span },
    LocalFuncDecl { annotations, async_marker, return_type, name, type_params, params, body, span },
    AssertStmt { condition, message, span },
    YieldStmt { is_star, value, span },
    ExprStmt { expr, span },
    RethrowStmt { span },
    EmptyStmt { span },
}
#[derive(Clone, Copy)]
pub struct Block<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for Block<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::Block).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> Block<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        if self.0.span().start == self.0.span().end
            && let Some(parent) = self.0.parent().filter(|p| p.kind() == NodeKind::FuncExpr)
        {
            return parent.span();
        }
        self.0.span()
    }
}
debug_node!(Block { stmts, span });
#[derive(Clone, Copy)]
pub struct ErrorNode<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for ErrorNode<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::ErrorNode).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> ErrorNode<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        self.0
            .children()
            .find(|node| node.kind() == NodeKind::Error)
            .and_then(|node| node.child::<Identifier>())
            .expect("parser emits original recovery span")
            .span()
    }
    pub fn message(self) -> &'a str {
        if self
            .0
            .parent()
            .is_some_and(|node| node.kind() == NodeKind::Program)
        {
            "unexpected token"
        } else {
            "could not parse class member"
        }
    }
}
debug_node!(ErrorNode { message, span });
typed_node!(SwitchCase, SwitchCase);
impl fmt::Debug for SwitchCase<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwitchCase")
            .field("labels", &super::macros::DebugList(self.labels()))
            .field("cases", &self.cases())
            .field("body", &self.body())
            .field("span", &self.span())
            .finish()
    }
}
#[derive(Clone, Copy)]
pub struct LocalVarDecl<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for LocalVarDecl<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::LocalVarDecl).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
debug_node!(LocalVarDecl {
    annotations,
    keyword,
    keyword_span,
    is_late,
    var_type,
    declarators,
    span,
    full_span
});
typed_node!(CaseCondition, CaseCondition);
typed_node!(ForInInit, ForInInit);
typed_node!(PatternForInInit, PatternForInInit);
typed_node!(ForExprsInit, ForExprsInit);
typed_node!(CasePatternLabel, CasePatternLabel);
typed_node!(CaseExprLabel, CaseExprLabel);
typed_node!(DefaultLabel, DefaultLabel);

impl<'a> Block<'a> {
    pub fn stmts(self) -> AstChildren<'a, Stmt<'a>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy)]
pub enum IfCondition<'a> {
    Expr(Expr<'a>),
    Case(CaseCondition<'a>),
}
impl<'a> AstNode<'a> for IfCondition<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        CaseCondition::cast(node)
            .map(Self::Case)
            .or_else(|| Expr::cast(node).map(Self::Expr))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Expr(v) => v.syntax(),
            Self::Case(v) => v.syntax(),
        }
    }
}
impl<'a> IfCondition<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        self.syntax().span()
    }
}
impl fmt::Debug for IfCondition<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expr(v) => f.debug_tuple("Expr").field(v).finish(),
            Self::Case(v) => fmt::Debug::fmt(v, f),
        }
    }
}
impl<'a> CaseCondition<'a> {
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn guard(self) -> Option<Expr<'a>> {
        self.0.children().filter_map(Expr::cast).nth(1)
    }
}
debug_tuple!(CaseCondition => "Case" { expr, pattern, guard });
impl<'a> IfStmt<'a> {
    pub fn condition(self) -> IfCondition<'a> {
        self.0.required()
    }
    pub fn then_branch(self) -> Stmt<'a> {
        self.0.required()
    }
    pub fn else_branch(self) -> Option<Stmt<'a>> {
        self.0.children().filter_map(Stmt::cast).nth(1)
    }
}
impl<'a> ForStmt<'a> {
    pub fn is_await(self) -> bool {
        self.0
            .children()
            .any(|node| node.kind() == NodeKind::AwaitFor)
    }
    pub fn header(self) -> ForHeader<'a> {
        self.0.required()
    }
    pub fn body(self) -> Stmt<'a> {
        self.0.required()
    }
}
#[derive(Clone, Copy)]
pub enum ForInit<'a> {
    VarDecl(LocalVarDecl<'a>),
    ForIn(ForInInit<'a>),
    PatternForIn(PatternForInInit<'a>),
    Exprs(ForExprsInit<'a>),
}
impl<'a> AstNode<'a> for ForInit<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        match node.kind() {
            NodeKind::LocalVarDecl => Some(Self::VarDecl(LocalVarDecl(node))),
            NodeKind::ForInInit => Some(Self::ForIn(ForInInit(node))),
            NodeKind::PatternForInInit => Some(Self::PatternForIn(PatternForInInit(node))),
            NodeKind::ForExprsInit => Some(Self::Exprs(ForExprsInit(node))),
            _ => None,
        }
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::VarDecl(v) => v.syntax(),
            Self::ForIn(v) => v.syntax(),
            Self::PatternForIn(v) => v.syntax(),
            Self::Exprs(v) => v.syntax(),
        }
    }
}
impl<'a> ForInit<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        self.syntax().span()
    }
}
impl fmt::Debug for ForInit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VarDecl(v) => f.debug_tuple("VarDecl").field(v).finish(),
            Self::ForIn(v) => fmt::Debug::fmt(v, f),
            Self::PatternForIn(v) => fmt::Debug::fmt(v, f),
            Self::Exprs(v) => f.debug_tuple("Exprs").field(&v.exprs()).finish(),
        }
    }
}
debug_node!(ForInInit => "ForIn" { annotations, declares_variable, keyword, var_type, name, iterable });
debug_node!(PatternForInInit => "PatternForIn" { annotations, keyword, pattern, iterable });
debug_tuple!(ForExprsInit => "Exprs" { exprs });
impl<'a> ForInInit<'a> {
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn declares_variable(self) -> bool {
        self.var_type().is_some() || keyword_token(self.0).is_some()
    }
    pub fn keyword(self) -> VarKeyword {
        keyword(self.0)
    }
    pub fn var_type(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    pub fn iterable(self) -> Expr<'a> {
        self.0
            .children()
            .filter_map(Expr::cast)
            .next_back()
            .expect("parser emits for-in iterable")
    }
}
impl<'a> PatternForInInit<'a> {
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn keyword(self) -> VarKeyword {
        keyword(self.0)
    }
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn iterable(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> ForExprsInit<'a> {
    pub fn exprs(self) -> AstChildren<'a, Expr<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> ForHeader<'a> {
    pub fn init(self) -> Option<ForInit<'a>> {
        self.0.child()
    }
    pub fn condition(self) -> Option<Expr<'a>> {
        self.0
            .children()
            .find(|n| n.kind() == NodeKind::ForCondition)
            .and_then(SyntaxNode::child)
    }
    pub fn updates(self) -> AstChildren<'a, Expr<'a>> {
        AstChildren::new(
            self.0
                .children()
                .find(|n| n.kind() == NodeKind::ForUpdates)
                .unwrap_or(self.0),
        )
    }
}
macro_rules! loop_accessors {
    ($($ty:ident),*) => { $(impl<'a> $ty<'a> {
        pub fn condition(self) -> Expr<'a> { self.0.required() }
        pub fn body(self) -> Stmt<'a> { self.0.required() }
    })* };
}
loop_accessors!(WhileStmt, DoWhileStmt);
impl<'a> SwitchStmt<'a> {
    pub fn subject(self) -> Expr<'a> {
        self.0.required()
    }
    pub fn cases(self) -> AstChildren<'a, SwitchCase<'a>> {
        AstChildren::new(self.0)
    }
}
impl<'a> SwitchCase<'a> {
    pub fn labels(self) -> impl Iterator<Item = Identifier<'a>> + Clone {
        AstChildren::<Identifier>::new(self.0)
    }
    pub fn cases(self) -> AstChildren<'a, SwitchCaseKind<'a>> {
        AstChildren::new(self.0)
    }
    pub fn body(self) -> AstChildren<'a, Stmt<'a>> {
        AstChildren::new(self.0)
    }
}
#[derive(Clone, Copy)]
pub enum SwitchCaseKind<'a> {
    Pattern(CasePatternLabel<'a>),
    Expr(CaseExprLabel<'a>),
    Default(DefaultLabel<'a>),
}
impl<'a> AstNode<'a> for SwitchCaseKind<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        match node.kind() {
            NodeKind::CasePatternLabel => Some(Self::Pattern(CasePatternLabel(node))),
            NodeKind::CaseExprLabel => Some(Self::Expr(CaseExprLabel(node))),
            NodeKind::DefaultLabel => Some(Self::Default(DefaultLabel(node))),
            _ => None,
        }
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Pattern(v) => v.syntax(),
            Self::Expr(v) => v.syntax(),
            Self::Default(v) => v.syntax(),
        }
    }
}
impl<'a> SwitchCaseKind<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        self.syntax().span()
    }
}
impl fmt::Debug for SwitchCaseKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pattern(v) => fmt::Debug::fmt(v, f),
            Self::Expr(v) => fmt::Debug::fmt(v, f),
            Self::Default(_) => f.write_str("Default"),
        }
    }
}
impl<'a> CasePatternLabel<'a> {
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn guard(self) -> Option<Expr<'a>> {
        self.0.child()
    }
}
impl<'a> CaseExprLabel<'a> {
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
debug_tuple!(CasePatternLabel => "Pattern" { pattern, guard });
debug_tuple!(CaseExprLabel => "Expr" { expr });
impl fmt::Debug for DefaultLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Default")
    }
}
impl<'a> TryCatchStmt<'a> {
    pub fn body(self) -> Block<'a> {
        self.0.required()
    }
    pub fn catches(self) -> AstChildren<'a, CatchClause<'a>> {
        AstChildren::new(self.0)
    }
    pub fn finally(self) -> Option<Block<'a>> {
        self.0.children().filter_map(Block::cast).nth(1)
    }
}
impl<'a> CatchClause<'a> {
    pub fn exception_type(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn exception_var(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>()
    }
    pub fn stack_trace_var(self) -> Option<Identifier<'a>> {
        AstChildren::<Identifier>::new(self.0).nth(1)
    }
    pub fn body(self) -> Block<'a> {
        self.0.required()
    }
}
impl<'a> ReturnStmt<'a> {
    pub fn value(self) -> Option<Expr<'a>> {
        self.0.child()
    }
}
impl<'a> ThrowStmt<'a> {
    pub fn value(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> BreakStmt<'a> {
    pub fn label(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>()
    }
}
impl<'a> ContinueStmt<'a> {
    pub fn label(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>()
    }
}
impl<'a> LocalVarDecl<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        let span = self.0.span();
        if self
            .0
            .parent()
            .is_some_and(|p| p.kind() == NodeKind::ForHeader)
        {
            let start = self.0.required::<VarDeclarator<'a>>().name().span().start;
            Span::new(start, span.end.max(start))
        } else {
            span
        }
    }
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn keyword(self) -> VarKeyword {
        keyword(self.0)
    }
    pub fn keyword_span(self) -> Option<Span> {
        keyword_token(self.0).map(SyntaxToken::span)
    }
    pub fn is_late(self) -> bool {
        self.0.child_token(TokenKind::Late).is_some()
    }
    pub fn var_type(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn declarators(self) -> AstChildren<'a, VarDeclarator<'a>> {
        AstChildren::new(self.0)
    }
    pub fn full_span(self) -> Span {
        if self
            .0
            .parent()
            .is_some_and(|p| p.kind() == NodeKind::ForHeader)
        {
            return self.span();
        }
        let span = self.span();
        let start = self
            .0
            .first_token()
            .and_then(|t| t.prev_significant())
            .map_or(0, |t| t.span().end);
        Span::new(
            self.0
                .tree()
                .documentation_start(self.0.source(), Span::new(start, span.start))
                .unwrap_or(span.start),
            span.end,
        )
    }
}
impl<'a> PatternDeclaration<'a> {
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn keyword(self) -> VarKeyword {
        keyword(self.0)
    }
    pub fn keyword_span(self) -> Span {
        keyword_token(self.0)
            .expect("parser emits pattern declaration keyword")
            .span()
    }
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn init(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> PatternAssignStmt<'a> {
    pub fn pattern(self) -> Pattern<'a> {
        self.0.required()
    }
    pub fn value(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> LabeledStmt<'a> {
    pub fn label(self) -> Identifier<'a> {
        self.0.required()
    }
    pub fn stmt(self) -> Stmt<'a> {
        self.0.required()
    }
}
impl<'a> LocalFuncDecl<'a> {
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn async_marker(self) -> AsyncMarker {
        let asynchronous = self.0.child_token(TokenKind::Async).is_some();
        let star = self.0.child_token(TokenKind::Star).is_some();
        match (asynchronous, star) {
            (true, true) => AsyncMarker::AsyncStar,
            (true, false) => AsyncMarker::Async,
            (false, true) => AsyncMarker::SyncStar,
            (false, false) => AsyncMarker::Sync,
        }
    }
    pub fn return_type(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        AstChildren::new(self.0)
    }
    pub fn params(self) -> FormalParamList<'a> {
        self.0.required()
    }
    pub fn body(self) -> FunctionBody<'a> {
        self.0.required()
    }
}
impl<'a> AssertStmt<'a> {
    pub fn condition(self) -> Expr<'a> {
        self.0.required()
    }
    pub fn message(self) -> Option<Expr<'a>> {
        self.0.children().filter_map(Expr::cast).nth(1)
    }
}
impl<'a> YieldStmt<'a> {
    pub fn is_star(self) -> bool {
        self.0.child_token(TokenKind::Star).is_some()
    }
    pub fn value(self) -> Expr<'a> {
        self.0.required()
    }
}
impl<'a> ExprStmt<'a> {
    pub fn expr(self) -> Expr<'a> {
        self.0.required()
    }
}
