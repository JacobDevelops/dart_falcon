use falcon_analyze::{DeclarationIdentity, ResolvedType, SemanticModel, resolve_call_constructor};
use falcon_syntax::ast::{ArgList, DartType, Expr, Identifier, NamedType, Span};

pub(crate) fn widget_construction(expr: &Expr) -> Option<(&str, &ArgList, &Span)> {
    match expr {
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
        } => match callee.as_ref() {
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
    match expression {
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

/// A constructor invocation resolved to its class and constructor name (`"new"`
/// for the unnamed constructor). Unresolved callees yield `None`.
pub(crate) fn constructor_invocation<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str, &'a ArgList)> {
    match expression {
        Expr::New {
            dart_type: DartType::Named(named),
            constructor_name,
            args,
            ..
        } => {
            let (identity, constructor) = new_constructor(named, constructor_name.as_ref(), model)?;
            Some((identity, constructor, args))
        }
        Expr::Call { callee, args, .. } => {
            let (identity, constructor) = resolve_call_constructor(callee, model, false)?;
            Some((identity, constructor, args))
        }
        _ => None,
    }
}

/// The class and constructor name (`"new"` for the unnamed constructor) of a
/// `new`/`const` instance creation. The parser folds a constructor name not
/// preceded by type arguments into the type's segments (`new A.named()`
/// parses as type `A.named`), so a trailing segment that does not resolve as
/// part of the type is taken as the constructor name.
pub(crate) fn new_constructor<'a>(
    named: &'a NamedType,
    constructor_name: Option<&'a Identifier>,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str)> {
    let segments: Vec<&str> = named
        .segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect();
    if let Some(constructor) = constructor_name {
        return Some((model.resolve_dotted(&segments)?, &constructor.name));
    }
    if let Some(identity) = model.resolve_dotted(&segments) {
        return Some((identity, "new"));
    }
    let (constructor, type_segments) = segments.split_last()?;
    if type_segments.is_empty() {
        return None;
    }
    Some((model.resolve_dotted(type_segments)?, constructor))
}
