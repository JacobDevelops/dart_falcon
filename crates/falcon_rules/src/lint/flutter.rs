use falcon_analyze::{ResolvedType, SemanticModel, resolve_call_constructor};
use falcon_syntax::ast::{ArgList, DartType, Expr, Span};

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
