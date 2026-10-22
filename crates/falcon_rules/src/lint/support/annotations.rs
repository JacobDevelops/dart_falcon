//! Resolved SDK annotation identities shared by lint rules.

use falcon_analyze::{QualifiedName, SemanticModel};
use falcon_syntax::cst::Annotation;

pub(crate) fn is_override(annotation: Annotation<'_>, model: SemanticModel<'_>) -> bool {
    if annotation.args().is_some() || annotation.constructor_name().is_some() {
        return false;
    }
    let Some(name) = QualifiedName::from_identifiers(annotation.name()) else {
        return false;
    };
    let identity = model
        .resolve_value(name)
        .or_else(|| model.resolve_name(name))
        .or_else(|| model.resolve_imported_member(name));
    identity
        .as_ref()
        .is_some_and(|identity| identity.is_sdk("dart:core", "override"))
        || (identity.is_none()
            && model
                .resolve_sdk_member(name)
                .is_some_and(|identity| identity.is_sdk("dart:core", "override")))
}

pub(crate) fn has_override<'a>(
    annotations: impl IntoIterator<Item = Annotation<'a>>,
    model: SemanticModel<'_>,
) -> bool {
    annotations
        .into_iter()
        .any(|annotation| is_override(annotation, model))
}

pub(crate) fn is_deprecated_constant(annotation: Annotation<'_>, model: SemanticModel<'_>) -> bool {
    model.is_deprecated_constant(annotation)
}

pub(crate) fn is_deprecated_constructor(
    annotation: Annotation<'_>,
    model: SemanticModel<'_>,
) -> bool {
    model.is_deprecated_constructor(annotation)
}

pub(crate) fn has_deprecated<'a>(
    annotations: impl IntoIterator<Item = Annotation<'a>>,
    model: SemanticModel<'_>,
) -> bool {
    annotations
        .into_iter()
        .any(|annotation| model.is_deprecation(annotation))
}
