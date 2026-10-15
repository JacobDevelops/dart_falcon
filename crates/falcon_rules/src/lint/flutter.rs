use falcon_analyze::{
    DeclarationIdentity, ResolvedType, SemanticModel, resolve_call_constructor,
    resolve_new_constructor,
};
use falcon_syntax::ast::{ArgList, DartType, Expr, Span};

pub(crate) fn widget_construction(expr: &Expr) -> Option<(&str, &ArgList, &Span)> {
    match expr.unparenthesized() {
        Expr::New {
            dart_type: DartType::Named(named),
            args,
            span,
            ..
        } => named
            .segments
            .last()
            .map(|name| (name.name.as_str(), args, span)),
        Expr::Call {
            callee, args, span, ..
        } => match callee.unparenthesized() {
            Expr::Ident(name) => Some((name.name.as_str(), args, span)),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn construction<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(ResolvedType, &'a ArgList)> {
    match expression.unparenthesized() {
        Expr::New {
            dart_type, args, ..
        } => Some((model.resolve_type(dart_type).with_nullable(false), args)),
        Expr::Call { callee, args, .. } => {
            let (identity, _) = resolve_call_constructor(callee, model, false)?;
            Some((ResolvedType::interface_identity(identity), args))
        }
        _ => None,
    }
}

/// A constructor invocation resolved to its declaration and constructor name
/// (`"new"` for the unnamed constructor). Unresolved callees yield `None`.
pub(crate) fn constructor_invocation<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str, &'a ArgList)> {
    match expression.unparenthesized() {
        Expr::New {
            dart_type,
            constructor_name,
            args,
            ..
        } => {
            let (identity, constructor) =
                resolve_new_constructor(dart_type, constructor_name.as_ref(), model)?;
            Some((identity, constructor, args))
        }
        Expr::Call { callee, args, .. } => {
            let (identity, constructor) = resolve_call_constructor(callee, model, false)?;
            Some((identity, constructor, args))
        }
        _ => None,
    }
}

/// Constructor resolution through type aliases in its lexical value scope.
pub(crate) fn scoped_constructor_invocation<'a>(
    expression: &'a Expr,
    state: &crate::lint::semantic_scope::SemanticState<'_>,
) -> Option<(DeclarationIdentity, &'a str, &'a ArgList)> {
    if let Expr::Call { callee, .. } = expression.unparenthesized() {
        let segments = falcon_analyze::expression_segments(callee, false)?;
        let head = *segments.first()?;
        if state.environment.is_bound(head) || state.type_parameters.contains(head) {
            return None;
        }
    }
    let (identity, constructor, args) = constructor_invocation(expression, &state.model)?;
    Some((state.model.unaliased(identity), constructor, args))
}

/// The constructor name range, excluding `new`/`const` and its arguments.
pub(crate) fn constructor_span(expression: &Expr) -> Span {
    match expression.unparenthesized() {
        Expr::New {
            dart_type,
            constructor_name,
            ..
        } => Span {
            start: dart_type.span().start,
            end: constructor_name
                .as_ref()
                .map_or(dart_type.span().end, |name| name.span.end),
        },
        Expr::Call { callee, .. } => *callee.span(),
        _ => *expression.span(),
    }
}
