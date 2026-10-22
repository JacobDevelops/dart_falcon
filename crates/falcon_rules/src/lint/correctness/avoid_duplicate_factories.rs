//! Flags factories in one type with the same implementation, ignoring factory
//! names and alpha-renaming parameters by position. Local bindings and closures
//! are skipped rather than confusing shadowed names with constructor parameters.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{TokenKind, cst};

#[derive(Default)]
pub struct AvoidDuplicateFactories;

impl Rule for AvoidDuplicateFactories {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, node: cst::TopLevelDecl<'a>) -> StructuralTraversal {
        if let Some(members) = crate::lint::declarations::members_of(node) {
            for (index, member) in members.clone().enumerate() {
                let cst::ClassMember::Constructor(constructor) = member else { continue };
                if !constructor.is_factory() { continue; }
                let Some(implementation) = implementation_of(constructor) else { continue };
                let earlier = members.clone().take(index).find_map(|member| {
                    let cst::ClassMember::Constructor(other) = member else { return None };
                    if !other.is_factory() || constructor.is_const() != other.is_const() { return None; }
                    let other_body = implementation_of(other)?;
                    normalized(implementation, constructor).eq(normalized(other_body, other)).then_some(other)
                });
                if let Some(earlier) = earlier {
                    let original = earlier.constructor_name().map_or_else(
                        || earlier.name().name().to_owned(),
                        |name| format!("{}.{}", earlier.name().name(), name.name()),
                    );
                    self.diags.push(self.ctx.diagnostic(
                        format!("This factory has the same implementation as `{original}` — remove it or add unique functionality."),
                        constructor.constructor_name().map_or(constructor.name().span(), |name| name.span()),
                    ));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}

fn implementation_of(constructor: cst::ConstructorDecl<'_>) -> Option<cst::SyntaxNode<'_>> {
    let node = if let Some(redirect) = constructor.redirect() {
        redirect.syntax()
    } else {
        match constructor.body()? {
            cst::FunctionBody::Arrow(body) => body.expr().syntax(),
            cst::FunctionBody::Block(body) => {
                let mut statements = body.stmts();
                match (statements.next(), statements.next()) {
                    (Some(cst::Stmt::Return(statement)), None) => statement.value()?.syntax(),
                    _ => body.syntax(),
                }
            }
            cst::FunctionBody::Native(_) => return None,
        }
    };
    if node.descendants().any(|node| {
        matches!(
            node.kind(),
            cst::NodeKind::VarDeclarator
                | cst::NodeKind::FormalParam(_)
                | cst::NodeKind::TypeParam
                | cst::NodeKind::PatternDeclaration
                | cst::NodeKind::FuncExpr
                | cst::NodeKind::LocalFuncDecl
                | cst::NodeKind::SwitchExpr
        )
    }) {
        return None;
    }
    if node
        .descendants()
        .filter_map(cst::Stmt::cast)
        .any(|statement| {
            !matches!(
                statement,
                cst::Stmt::Block(_) | cst::Stmt::Return(_) | cst::Stmt::Expr(_)
            )
        })
    {
        return None;
    }
    Some(node)
}

enum Token<'a> {
    Parameter(usize, cst::FormalParam<'a>),
    Literal(TokenKind, &'a str),
}

impl<'a> PartialEq for Token<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Literal(left_kind, left), Self::Literal(right_kind, right)) => {
                left_kind == right_kind && left == right
            }
            (Self::Parameter(left_index, left), Self::Parameter(right_index, right)) => {
                let signature = |param: cst::FormalParam<'a>| {
                    param
                        .syntax()
                        .significant_tokens()
                        .filter(move |token| token.span() != param.name().span())
                        .map(|token| (token.kind(), token.text()))
                };
                left_index == right_index
                    && left.kind() == right.kind()
                    && signature(*left).eq(signature(*right))
            }
            _ => false,
        }
    }
}

fn normalized<'a>(
    node: cst::SyntaxNode<'a>,
    constructor: cst::ConstructorDecl<'a>,
) -> impl Iterator<Item = Token<'a>> {
    node.significant_tokens().map(move |token| {
        let member = token.prev_significant().is_some_and(|previous| {
            matches!(
                previous.kind(),
                TokenKind::Dot | TokenKind::QmarkDot | TokenKind::DotDot | TokenKind::DotDotQmark
            )
        });
        let label = token.parent().parent().is_some_and(|parent| {
            cst::NamedArg::cast(parent)
                .map(|argument| argument.name())
                .or_else(|| cst::RecordField::cast(parent).and_then(|field| field.name()))
                .is_some_and(|name| name.span() == token.span())
        });
        let type_name = token
            .parent()
            .ancestors()
            .any(|node| cst::DartType::cast(node).is_some());
        if !member
            && !label
            && !type_name
            && token.parent().kind() == cst::NodeKind::Identifier
            && let Some((index, parameter)) = constructor
                .params()
                .params()
                .enumerate()
                .find(|(_, param)| param.name().name() == token.text())
        {
            Token::Parameter(index, parameter)
        } else {
            Token::Literal(token.kind(), token.text())
        }
    })
}
