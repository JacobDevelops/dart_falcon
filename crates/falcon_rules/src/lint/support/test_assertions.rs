//! Shared filename and canonical package-call guards for test assertion rules.

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{AnalyzeContext, QualifiedName};
use falcon_syntax::cst::*;

const FRAMEWORK_PACKAGES: [&str; 4] = ["test", "test_core", "test_api", "flutter_test"];
pub(crate) const MATCHER_PACKAGES: [&str; 5] =
    ["test", "test_core", "test_api", "flutter_test", "matcher"];

pub(crate) fn enabled(program: Program<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    crate::lint::declarations::has_test_file_suffix(ctx.file_path())
        && program.imports().any(|import| {
            let uri = import.uri().value();
            uri.strip_prefix("package:")
                .and_then(|uri| uri.split_once('/'))
                .is_some_and(|(package, _)| is_framework(package))
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
    for package in MATCHER_PACKAGES.into_iter().chain(["mockito", "mocktail"]) {
        if identity.is_package(package, name) {
            return Some((name, package));
        }
    }
    None
}

pub(crate) fn is_framework(package: &str) -> bool {
    FRAMEWORK_PACKAGES.contains(&package)
}

pub(crate) fn is_assertion(name: &str, package: &str) -> bool {
    match name {
        "expect" | "expectLater" | "fail" => MATCHER_PACKAGES.contains(&package),
        "verify"
        | "verifyNever"
        | "verifyInOrder"
        | "verifyNoMoreInteractions"
        | "verifyZeroInteractions" => matches!(package, "mockito" | "mocktail"),
        _ => {
            MATCHER_PACKAGES.contains(&package)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_packages_share_assertion_classification() {
        for package in ["test", "test_core", "test_api", "flutter_test", "matcher"] {
            for name in [
                "expect",
                "expectLater",
                "fail",
                "expectAsync0",
                "expectAsyncUntil6",
            ] {
                assert!(is_assertion(name, package), "{package}: {name}");
            }
            assert!(!is_assertion("verify", package), "{package}");
        }
        assert!(!is_assertion("expect", "unrelated"));
        for package in ["mockito", "mocktail"] {
            assert!(is_assertion("verify", package));
            assert!(!is_assertion("expect", package));
        }
    }
}
