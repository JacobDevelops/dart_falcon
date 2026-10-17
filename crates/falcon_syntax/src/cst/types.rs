use std::fmt;

use crate::token::TokenKind;

use super::macros::DebugList;
use super::{Annotation, AstChildren, AstNode, Identifier, NodeKind, Span, SyntaxNode};

#[derive(Clone, Copy)]
pub enum DartType<'a> {
    Named(NamedType<'a>),
    Function(FunctionType<'a>),
    Record(RecordType<'a>),
    Void(VoidType<'a>),
    Dynamic(DynamicType<'a>),
}

impl<'a> AstNode<'a> for DartType<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        Some(match node.kind() {
            NodeKind::NamedType => Self::Named(NamedType(node)),
            NodeKind::FunctionType | NodeKind::LegacyTypedef | NodeKind::SetterParameter => {
                Self::Function(FunctionType::cast(node)?)
            }
            NodeKind::RecordType => Self::Record(RecordType(node)),
            NodeKind::VoidType => Self::Void(VoidType(node)),
            NodeKind::DynamicType => Self::Dynamic(DynamicType(node)),
            _ => return None,
        })
    }

    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Named(node) => node.syntax(),
            Self::Function(node) => node.syntax(),
            Self::Record(node) => node.syntax(),
            Self::Void(node) => node.syntax(),
            Self::Dynamic(node) => node.syntax(),
        }
    }
    fn span(self) -> Span {
        self.span()
    }
}

impl<'a> DartType<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        match self {
            Self::Function(node) => node.span(),
            _ => self.syntax().span(),
        }
    }
    pub fn is_nullable(self) -> bool {
        match self {
            Self::Named(node) => node.is_nullable(),
            Self::Function(node) => node.is_nullable(),
            Self::Record(node) => node.is_nullable(),
            Self::Void(_) | Self::Dynamic(_) => false,
        }
    }
}

impl fmt::Debug for DartType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(node) => f.debug_tuple("Named").field(node).finish(),
            Self::Function(node) => f.debug_tuple("Function").field(node).finish(),
            Self::Record(node) => f.debug_tuple("Record").field(node).finish(),
            Self::Void(node) => fmt::Debug::fmt(node, f),
            Self::Dynamic(node) => fmt::Debug::fmt(node, f),
        }
    }
}

typed_node!(NamedType, NamedType);
#[derive(Clone, Copy)]
pub struct FunctionType<'a>(pub(crate) SyntaxNode<'a>);

impl<'a> AstNode<'a> for FunctionType<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (matches!(
            node.kind(),
            NodeKind::FunctionType | NodeKind::LegacyTypedef
        ) || (node.kind() == NodeKind::SetterParameter
            && node
                .children()
                .any(|child| child.kind() == NodeKind::FormalParamList)))
        .then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}

#[derive(Clone, Copy)]
pub struct FunctionTypeParam<'a>(pub(crate) SyntaxNode<'a>);

impl<'a> AstNode<'a> for FunctionTypeParam<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        matches!(
            node.kind(),
            NodeKind::FunctionTypeParam | NodeKind::FormalParam
        )
        .then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
}
typed_node!(RecordType, RecordType);
typed_node!(NamedRecordField, NamedRecordField);
#[derive(Clone, Copy)]
pub struct TypeParam<'a>(pub(crate) SyntaxNode<'a>);

impl<'a> AstNode<'a> for TypeParam<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        if node.kind() == NodeKind::TypeParam {
            return Some(Self(node));
        }
        let args = node.parent()?;
        let head = args.parent()?;
        (args.kind() == NodeKind::TypeArgs
            && head.kind() == NodeKind::NamedType
            && head.parent()?.kind() == NodeKind::LegacyTypedef
            && !head
                .parent()?
                .children()
                .any(|child| child.kind() == NodeKind::Identifier)
            && DartType::cast(node).is_some())
        .then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
}
typed_node!(VoidType, VoidType);
typed_node!(DynamicType, DynamicType);
typed_node!(TypeArgs, TypeArgs);

impl<'a> TypeArgs<'a> {
    pub fn args(self) -> AstChildren<'a, DartType<'a>> {
        AstChildren::new(self.0)
    }

    /// Returns types inside the owner's immediate type-argument role, or an empty iterator.
    pub fn of(parent: SyntaxNode<'a>) -> AstChildren<'a, DartType<'a>> {
        let mut children = parent.children();
        match children.find(|node| node.kind() == NodeKind::TypeArgs) {
            Some(role) => AstChildren::new(role),
            None => AstChildren::from_children(children),
        }
    }
}

impl<'a> NamedType<'a> {
    pub fn segments(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        self.0.children().filter_map(Identifier::from_node)
    }
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    pub fn is_nullable(self) -> bool {
        self.0.child_token(TokenKind::Qmark).is_some()
    }
}

impl fmt::Debug for NamedType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedType")
            .field("segments", &DebugList(self.segments()))
            .field("type_args", &self.type_args())
            .field("is_nullable", &self.is_nullable())
            .field("span", &self.span())
            .finish()
    }
}

impl<'a> FunctionType<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        if self.0.kind() == NodeKind::LegacyTypedef {
            let end = self
                .0
                .children()
                .find(|node| node.kind() == NodeKind::FormalParamList)
                .expect("legacy typedef has formal parameters")
                .span()
                .end;
            Span::new(self.0.span().start, end)
        } else if self.0.kind() == NodeKind::SetterParameter {
            let start = self.return_type().map_or_else(
                || {
                    self.0
                        .children()
                        .find_map(Identifier::from_node)
                        .expect("setter parameter has a name")
                        .span()
                        .start
                },
                |ty| ty.span().start,
            );
            Span::new(start, self.0.span().end)
        } else {
            self.0.span()
        }
    }
    pub fn return_type(self) -> Option<DartType<'a>> {
        if self.0.kind() == NodeKind::LegacyTypedef
            && !self
                .0
                .children()
                .any(|node| Identifier::from_node(node).is_some())
        {
            return None;
        }
        self.0.children().find_map(DartType::cast)
    }
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        let mut children = self.0.children();
        if self.0.kind() == NodeKind::LegacyTypedef {
            children.by_ref().for_each(drop);
        }
        AstChildren::from_children(children)
    }
    pub fn params(self) -> AstChildren<'a, FunctionTypeParam<'a>> {
        let parent = self
            .0
            .children()
            .find(|node| node.kind() == NodeKind::FormalParamList)
            .unwrap_or(self.0);
        AstChildren::new(parent)
    }
    pub fn is_nullable(self) -> bool {
        self.0.kind() != NodeKind::LegacyTypedef && self.0.child_token(TokenKind::Qmark).is_some()
    }
}

debug_node!(FunctionType {
    return_type,
    type_params,
    params,
    is_nullable,
    span
});

impl<'a> FunctionTypeParam<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        self.0.span()
    }
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    pub fn name(self) -> Option<Identifier<'a>> {
        self.0.children().find_map(Identifier::from_node)
    }
    pub fn param_type(self) -> DartType<'a> {
        self.0
            .children()
            .find_map(DartType::cast)
            .unwrap_or_else(|| {
                assert_eq!(
                    self.0.kind(),
                    NodeKind::FormalParam,
                    "function type parameter always has a type"
                );
                DartType::Dynamic(DynamicType(self.0))
            })
    }
    fn group(self) -> Option<TokenKind> {
        self.0.children().find_map(|child| match child.kind() {
            NodeKind::NamedTypeParamFlag => Some(TokenKind::LBrace),
            NodeKind::OptionalTypeParamFlag => Some(TokenKind::LBracket),
            _ => None,
        })
    }
    pub fn is_named(self) -> bool {
        self.group() == Some(TokenKind::LBrace)
    }
    pub fn is_required(self) -> bool {
        (self.0.child_token(TokenKind::Required).is_some()
            && (self.0.kind() != NodeKind::FormalParam || self.is_named()))
            || self.group().is_none()
    }
}

debug_node!(FunctionTypeParam {
    annotations,
    name,
    param_type,
    is_required,
    is_named
});

impl<'a> RecordType<'a> {
    pub fn positional(self) -> AstChildren<'a, DartType<'a>> {
        AstChildren::new(self.0)
    }
    pub fn positional_names(self) -> impl Iterator<Item = Option<Identifier<'a>>> + Clone {
        let mut children = self.0.children().peekable();
        std::iter::from_fn(move || {
            while let Some(child) = children.next() {
                if DartType::cast(child).is_some() {
                    return Some(children.peek().copied().and_then(Identifier::from_node));
                }
            }
            None
        })
    }
    pub fn named(self) -> AstChildren<'a, NamedRecordField<'a>> {
        AstChildren::new(self.0)
    }
    pub fn is_nullable(self) -> bool {
        self.0.child_token(TokenKind::Qmark).is_some()
    }
}

impl fmt::Debug for RecordType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordType")
            .field("positional", &self.positional())
            .field("positional_names", &DebugList(self.positional_names()))
            .field("named", &self.named())
            .field("is_nullable", &self.is_nullable())
            .field("span", &self.span())
            .finish()
    }
}

impl<'a> NamedRecordField<'a> {
    pub fn name(self) -> Identifier<'a> {
        self.0
            .children()
            .find_map(Identifier::from_node)
            .expect("named record field always has a name")
    }
    pub fn field_type(self) -> DartType<'a> {
        self.0
            .children()
            .find_map(DartType::cast)
            .expect("named record field always has a type")
    }
}

debug_node!(NamedRecordField { name, field_type });

impl<'a> TypeParam<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        self.0.span()
    }
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        let mut children = self.0.children();
        if self.0.kind() != NodeKind::TypeParam {
            children.by_ref().for_each(drop);
        }
        AstChildren::from_children(children)
    }
    pub fn name(self) -> Identifier<'a> {
        if matches!(self.0.kind(), NodeKind::TypeParam | NodeKind::NamedType)
            && let Some(name) = self
                .0
                .children()
                .filter_map(Identifier::from_node)
                .next_back()
        {
            return name;
        }
        Identifier::synthetic(self.0, "<error>", self.span())
    }
    pub fn bound(self) -> Option<DartType<'a>> {
        (self.0.kind() == NodeKind::TypeParam)
            .then(|| self.0.children().find_map(DartType::cast))
            .flatten()
    }
}

debug_node!(TypeParam {
    annotations,
    name,
    bound,
    span
});
debug_node!(VoidType => "Void" { span });
debug_node!(DynamicType => "Dynamic" { span });
debug_node!(TypeArgs { args });
