//! Shared filename and canonical package-call guards for test assertion rules.

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{AnalyzeContext, QualifiedName};
use falcon_syntax::cst::*;

pub(crate) fn enabled(program: Program<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    crate::lint::declarations::has_test_file_suffix(ctx.file_path)
        && program.imports().any(|import| {
            let uri = import.uri().value();
            uri.starts_with("package:test/") || uri.starts_with("package:flutter_test/")
        })
}

/// Resolve only top-level package functions, never local or receiver methods.
pub(crate) fn package_call<'a>(
    call: CallExpr<'a>,
    state: &SemanticState<'a>,
) -> Option<(&'a str, &'static str)> {
    let (qualified, name) = match call.callee().unparenthesized() {
        Expr::Ident(name) => {
            if state.environment.is_bound(name.name())
                || state.enclosing_type.as_ref().is_some_and(|receiver| {
                    state
                        .signatures
                        .resolved_member(receiver, name.name(), &state.model)
                        .is_some()
                })
            {
                return None;
            }
            (QualifiedName::from(name.name()), name.name())
        }
        Expr::Field(field) if !field.is_null_safe() => {
            let Expr::Ident(prefix) = field.object().unparenthesized() else {
                return None;
            };
            if state.environment.is_bound(prefix.name()) {
                return None;
            }
            (
                QualifiedName::prefixed(prefix.name(), field.field().name()),
                field.field().name(),
            )
        }
        _ => return None,
    };
    let identity = state.model.resolve_value(qualified)?;
    for package in [
        "test",
        "test_core",
        "test_api",
        "flutter_test",
        "matcher",
        "mockito",
        "mocktail",
    ] {
        if identity.is_package(package, name) {
            return Some((name, package));
        }
    }
    None
}

pub(crate) fn is_framework(package: &str) -> bool {
    matches!(package, "test" | "test_core" | "test_api" | "flutter_test")
}

pub(super) fn is_assertion(name: &str, package: &str) -> bool {
    match name {
        "expect" | "expectLater" | "fail" => is_framework(package) || package == "matcher",
        "verify"
        | "verifyNever"
        | "verifyInOrder"
        | "verifyNoMoreInteractions"
        | "verifyZeroInteractions" => matches!(package, "mockito" | "mocktail"),
        _ => {
            (is_framework(package) || package == "matcher")
                && matches!(
                    name,
                    "expectAsync0"
                        | "expectAsync1"
                        | "expectAsync2"
                        | "expectAsync3"
                        | "expectAsync4"
                        | "expectAsync5"
                        | "expectAsync6"
                        | "expectAsyncUntil0"
                        | "expectAsyncUntil1"
                        | "expectAsyncUntil2"
                        | "expectAsyncUntil3"
                        | "expectAsyncUntil4"
                        | "expectAsyncUntil5"
                        | "expectAsyncUntil6"
                )
        }
    }
}
