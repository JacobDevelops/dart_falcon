use std::fmt;

use crate::token::TokenKind;

use super::macros::DebugList;
use super::{
    Annotation, AstChildren, AstNode, FormalParamList, Identifier, NodeKind, Span, SyntaxNode,
};

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
    #[must_use]
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    #[must_use]
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    #[must_use]
    pub fn span(self) -> Span {
        match self {
            Self::Function(node) => node.span(),
            _ => self.syntax().span(),
        }
    }
    #[must_use]
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

typed_node!(
    FunctionTypeParam,
    pattern NodeKind::FunctionTypeParam(_) | NodeKind::FormalParam(_)
);
typed_node!(RecordType, RecordType);
typed_node!(NamedRecordField, NamedRecordField);
#[derive(Clone, Copy)]
pub struct TypeParam<'a>(pub(crate) SyntaxNode<'a>);

impl<'a> AstNode<'a> for TypeParam<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::TypeParam).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
}
typed_node!(VoidType, VoidType);
typed_node!(DynamicType, DynamicType);
typed_node!(TypeArgs, TypeArgs);

impl<'a> TypeArgs<'a> {
    #[must_use]
    pub fn args(self) -> AstChildren<'a, DartType<'a>> {
        AstChildren::new(self.0)
    }

    /// Returns types inside the owner's immediate type-argument role, or an empty iterator.
    #[must_use]
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
        AstChildren::<Identifier>::new(self.0)
    }
    #[must_use]
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    #[must_use]
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
        if self.0.kind() == NodeKind::LegacyTypedef {
            let end = self.0.required::<FormalParamList>().span().end;
            Span::new(self.0.span().start, end)
        } else if self.0.kind() == NodeKind::SetterParameter {
            let start = self.return_type().map_or_else(
                || self.0.required::<Identifier>().span().start,
                |ty| ty.span().start,
            );
            Span::new(start, self.0.span().end)
        } else {
            self.0.span()
        }
    }
    #[must_use]
    pub fn return_type(self) -> Option<DartType<'a>> {
        self.0.child()
    }
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        let mut children = self.0.children();
        if self.0.kind() == NodeKind::LegacyTypedef {
            children.by_ref().for_each(drop);
        }
        AstChildren::from_children(children)
    }
    #[must_use]
    pub fn params(self) -> AstChildren<'a, FunctionTypeParam<'a>> {
        let parent = self
            .0
            .children()
            .find(|node| node.kind() == NodeKind::FormalParamList)
            .unwrap_or(self.0);
        AstChildren::new(parent)
    }
    #[must_use]
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
    #[must_use]
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
    pub fn name(self) -> Option<Identifier<'a>> {
        self.0.child::<Identifier>()
    }
    #[must_use]
    pub fn param_type(self) -> DartType<'a> {
        self.0
            .child()
            .unwrap_or(DartType::Dynamic(DynamicType(self.0)))
    }
    #[must_use]
    pub fn kind(self) -> super::ParamKind {
        self.0.param_kind()
    }
    #[must_use]
    pub fn is_named(self) -> bool {
        matches!(self.kind(), super::ParamKind::Named { .. })
    }
    #[must_use]
    pub fn is_required(self) -> bool {
        matches!(
            self.kind(),
            super::ParamKind::Positional | super::ParamKind::Named { required: true }
        )
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
    #[must_use]
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
    #[must_use]
    pub fn named(self) -> AstChildren<'a, NamedRecordField<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
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
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required::<Identifier>()
    }
    #[must_use]
    pub fn field_type(self) -> DartType<'a> {
        self.0.required()
    }
}

debug_node!(NamedRecordField { name, field_type });

impl<'a> TypeParam<'a> {
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
        self.0.span()
    }
    #[must_use]
    pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.0)
    }
    #[must_use]
    pub fn name(self) -> Identifier<'a> {
        self.0.required()
    }
    #[must_use]
    pub fn bound(self) -> Option<DartType<'a>> {
        self.0.child()
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
