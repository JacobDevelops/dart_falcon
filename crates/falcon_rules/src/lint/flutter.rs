use falcon_analyze::{
    DeclarationIdentity, ResolvedType, SemanticModel, resolve_call_constructor,
    resolve_new_constructor,
};
use falcon_syntax::cst::{ArgList, DartType, Expr, Span};

pub(crate) fn widget_construction<'a>(expr: Expr<'a>) -> Option<(&'a str, ArgList<'a>, Span)> {
    match expr.unparenthesized() {
        Expr::New(node) => {
            let DartType::Named(named) = node.dart_type() else {
                return None;
            };
            named
                .segments()
                .last()
                .map(|name| (name.name(), node.args(), node.span()))
        }
        Expr::Call(node) => match node.callee().unparenthesized() {
            Expr::Ident(name) => Some((name.name(), node.args(), node.span())),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn construction<'a>(
    expression: Expr<'a>,
    model: &SemanticModel<'_>,
) -> Option<(ResolvedType, ArgList<'a>)> {
    match expression.unparenthesized() {
        Expr::New(node) => Some((
            model.resolve_type(node.dart_type()).with_nullable(false),
            node.args(),
        )),
        Expr::Call(node) => {
            let (identity, _) = resolve_call_constructor(node.callee(), model, false)?;
            Some((ResolvedType::interface_identity(identity), node.args()))
        }
        _ => None,
    }
}

/// A constructor invocation resolved to its declaration and constructor name
/// (`"new"` for the unnamed constructor). Unresolved callees yield `None`.
pub(crate) fn constructor_invocation<'a>(
    expression: Expr<'a>,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str, ArgList<'a>)> {
    match expression.unparenthesized() {
        Expr::New(node) => {
            let (identity, constructor) =
                resolve_new_constructor(node.dart_type(), node.constructor_name(), model)?;
            Some((identity, constructor, node.args()))
        }
        Expr::Call(node) => {
            let (identity, constructor) = resolve_call_constructor(node.callee(), model, false)?;
            Some((identity, constructor, node.args()))
        }
        _ => None,
    }
}

/// Constructor resolution through type aliases in its lexical value scope.
pub(crate) fn scoped_constructor_invocation<'a>(
    expression: Expr<'a>,
    state: &falcon_analyze::semantic_scope::SemanticState<'_>,
) -> Option<(DeclarationIdentity, &'a str, ArgList<'a>)> {
    if let Expr::Call(node) = expression.unparenthesized() {
        let segments = falcon_analyze::expression_segments(node.callee(), false)?;
        let head = *segments.first()?;
        if state.environment.is_bound(head) || state.type_parameters.contains(head) {
            return None;
        }
    }
    let (identity, constructor, args) = constructor_invocation(expression, &state.model)?;
    Some((state.model.unaliased(identity), constructor, args))
}

/// The constructor name range, excluding `new`/`const` and its arguments.
pub(crate) fn constructor_span(expression: Expr<'_>) -> Span {
    match expression.unparenthesized() {
        Expr::New(node) => Span {
            start: node.dart_type().span().start,
            end: node
                .constructor_name()
                .map_or(node.dart_type().span().end, |name| name.span().end),
        },
        Expr::Call(node) => node.callee().span(),
        _ => expression.span(),
    }
}
