//! Reject platform-dependent runtime checks involving JavaScript interop types.

use std::collections::HashSet;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel,
    SignatureIndex, Truth, TypeParameterId, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct InvalidRuntimeCheckWithJsInteropTypes;

impl Rule for InvalidRuntimeCheckWithJsInteropTypes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, RuntimeChecks, ctx, diagnostics)
    }
}

#[derive(Clone)]
struct Normalized {
    ty: ResolvedType,
    interop: Interop,
}

#[derive(Clone, PartialEq, Eq)]
enum Interop {
    None,
    Sdk,
    User(DeclarationIdentity),
    Unknown,
}

struct Collector<'borrow, 'source> {
    diagnostics: &'borrow mut Vec<Diagnostic>,
    ctx: &'borrow AnalyzeContext<'source>,
    model: &'borrow SemanticModel<'source>,
    signatures: &'borrow SignatureIndex,
}

impl Collector<'_, '_> {
    fn normalize(&self, ty: &ResolvedType) -> Normalized {
        self.normalize_inner(ty, &mut HashSet::new())
    }

    fn normalize_inner(
        &self,
        ty: &ResolvedType,
        visiting: &mut HashSet<DeclarationIdentity>,
    ) -> Normalized {
        match ty.view() {
            ResolvedTypeView::Unknown => Normalized {
                ty: ResolvedType::UNKNOWN,
                interop: Interop::Unknown,
            },
            ResolvedTypeView::TypeParameter { bound, .. } => self.normalize_inner(bound, visiting),
            ResolvedTypeView::NonNullTypeParameter { bound, .. } => {
                self.normalize_inner(&bound.non_null(), visiting)
            }
            ResolvedTypeView::Interface {
                identity,
                extension_type,
                ..
            } => {
                if let DeclarationIdentity::Sdk { library, .. } = identity
                    && library.as_str() == "dart:js_interop"
                {
                    return Normalized {
                        ty: js_type(identity.clone()),
                        interop: Interop::Sdk,
                    };
                }
                if !visiting.insert(identity.clone()) {
                    return Normalized {
                        ty: ResolvedType::UNKNOWN,
                        interop: Interop::Unknown,
                    };
                }
                let facts = self.signatures.declaration(identity);
                let static_interop = facts.is_some_and(|facts| {
                    has_annotation(&facts.annotations, "JS")
                        && has_annotation(&facts.annotations, "staticInterop")
                });
                let representation =
                    facts.and_then(|facts| facts.extension_representation.as_ref());
                let result = if static_interop {
                    Normalized {
                        ty: js_type(DeclarationIdentity::sdk("dart:js_interop", "JSObject")),
                        interop: Interop::User(identity.clone()),
                    }
                } else if extension_type || representation.is_some() {
                    representation.map_or(
                        Normalized {
                            ty: ResolvedType::UNKNOWN,
                            interop: Interop::Unknown,
                        },
                        |representation| {
                            let substitutions = self
                                .signatures
                                .type_parameters(identity)
                                .unwrap_or_default()
                                .iter()
                                .cloned()
                                .zip(ty.type_arguments())
                                .collect();
                            let representation =
                                self.model.substitute(representation, &substitutions);
                            let normalized = self.normalize_inner(&representation, visiting);
                            Normalized {
                                ty: normalized.ty,
                                interop: match normalized.interop {
                                    Interop::Sdk | Interop::User(_) => {
                                        Interop::User(identity.clone())
                                    }
                                    other => other,
                                },
                            }
                        },
                    )
                } else {
                    Normalized {
                        ty: ty
                            .map_children(|child| self.normalize_inner(child, visiting).ty)
                            .with_nullable(false),
                        interop: Interop::None,
                    }
                };
                visiting.remove(identity);
                result
            }
            ResolvedTypeView::Function { .. } | ResolvedTypeView::Record { .. } => Normalized {
                ty: ty
                    .map_children(|child| self.normalize_inner(child, visiting).ty)
                    .with_nullable(false),
                interop: Interop::None,
            },
            // `promoteToNonNull(Null)` is `Never`.
            ResolvedTypeView::Null => Normalized {
                ty: ResolvedType::NEVER,
                interop: Interop::None,
            },
            _ => Normalized {
                ty: ty.with_nullable(false),
                interop: Interop::None,
            },
        }
    }

    /// The analyzer's `InteropTypeChecker.hasInteropType`: whether `ty` is or
    /// contains (in type arguments, function and record types, or type
    /// parameter bounds) a JS interop type. `None` when a part is unresolved.
    fn has_interop(
        &self,
        ty: &ResolvedType,
        visiting: &mut HashSet<TypeParameterId>,
    ) -> Option<bool> {
        match ty.view() {
            ResolvedTypeView::Unknown => None,
            ResolvedTypeView::TypeParameter { id, bound, .. }
            | ResolvedTypeView::NonNullTypeParameter { id, bound } => {
                if !visiting.insert(id.clone()) {
                    return Some(false);
                }
                self.has_interop(bound, visiting)
            }
            ResolvedTypeView::Interface { arguments, .. } => {
                match self.normalize(ty).interop {
                    Interop::Sdk | Interop::User(_) => return Some(true),
                    Interop::Unknown => return None,
                    Interop::None => {}
                }
                self.any_has_interop(arguments.iter(), visiting)
            }
            ResolvedTypeView::Function {
                return_type,
                positional,
                named,
                ..
            } => Some(
                self.has_interop(return_type, visiting)?
                    || self.any_has_interop(positional.iter(), visiting)?
                    || self.any_has_interop(named.iter().map(|(_, ty)| ty), visiting)?,
            ),
            ResolvedTypeView::Record {
                positional, named, ..
            } => Some(
                self.any_has_interop(positional.iter(), visiting)?
                    || self.any_has_interop(named.iter().map(|(_, ty)| ty), visiting)?,
            ),
            _ => Some(false),
        }
    }

    fn any_has_interop<'t>(
        &self,
        types: impl IntoIterator<Item = &'t ResolvedType>,
        visiting: &mut HashSet<TypeParameterId>,
    ) -> Option<bool> {
        for ty in types {
            if self.has_interop(ty, visiting)? {
                return Some(true);
            }
        }
        Some(false)
    }

    fn invalid_test(&self, left: &ResolvedType, right: &ResolvedType, is_check: bool) -> bool {
        let left_normalized = self.normalize(left);
        let right_normalized = self.normalize(right);
        if matches!(left_normalized.interop, Interop::Unknown)
            || matches!(right_normalized.interop, Interop::Unknown)
        {
            return false;
        }
        let has_direct_interop = !matches!(left_normalized.interop, Interop::None)
            || !matches!(right_normalized.interop, Interop::None);
        if has_direct_interop {
            let left_subtype = self.is_subtype(&left_normalized.ty, &right_normalized.ty);
            let right_subtype = self.is_subtype(&right_normalized.ty, &left_normalized.ty);
            if is_check {
                if !left_subtype && !matches!(right_normalized.ty.view(), ResolvedTypeView::Dynamic) {
                    return true;
                }
                if left_subtype
                    && !matches!(left_normalized.interop, Interop::None)
                    && matches!(right_normalized.interop, Interop::User(_))
                    && !self.original_subtype(left, right)
                {
                    return true;
                }
            } else if !left_subtype
                && !right_subtype
                && !matches!(left_normalized.ty.view(), ResolvedTypeView::Dynamic)
                && !matches!(right_normalized.ty.view(), ResolvedTypeView::Dynamic)
            {
                return true;
            }
        }
        self.invalid_nested(left, right, is_check)
    }

    /// Type arguments and record fields are compared pairwise, as the
    /// analyzer's `canBeSubtypeOf` does; it never descends into function
    /// types, so their parameters and return types are not checked.
    fn invalid_nested(&self, left: &ResolvedType, right: &ResolvedType, is_check: bool) -> bool {
        match (left.view(), right.view()) {
            (
                ResolvedTypeView::Interface {
                    arguments: left, ..
                },
                ResolvedTypeView::Interface {
                    arguments: right, ..
                },
            ) if left.len() == right.len() => left
                .iter()
                .zip(right.iter())
                .any(|(left, right)| self.invalid_test(left, right, is_check)),
            (
                ResolvedTypeView::Record {
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedTypeView::Record {
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                paired_invalid(self, left_positional, right_positional, is_check)
                    || named_invalid(self, left_named, right_named, is_check)
            }
            (
                ResolvedTypeView::TypeParameter { bound, .. }
                | ResolvedTypeView::NonNullTypeParameter { bound, .. },
                _,
            ) => self.invalid_test(bound, right, is_check),
            (
                _,
                ResolvedTypeView::TypeParameter { bound, .. }
                | ResolvedTypeView::NonNullTypeParameter { bound, .. },
            ) => self.invalid_test(left, bound, is_check),
            _ => false,
        }
    }

    fn is_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        if matches!(left.view(), ResolvedTypeView::Never) || matches!(right.view(), ResolvedTypeView::Dynamic) {
            return true;
        }
        // `dynamic` is a subtype of top types only; operands are non-nullable.
        if matches!(left.view(), ResolvedTypeView::Dynamic) {
            return false;
        }
        if let ResolvedTypeView::Interface { identity, .. } = right.view()
            && *identity == DeclarationIdentity::OBJECT
        {
            return true;
        }
        match (left.view(), right.view()) {
            (
                ResolvedTypeView::Interface {
                    identity: left_identity,
                    arguments: left_arguments,
                    ..
                },
                ResolvedTypeView::Interface {
                    identity: right_identity,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                if left_identity == right_identity {
                    return left_arguments.len() == right_arguments.len()
                        && left_arguments
                            .iter()
                            .zip(right_arguments.iter())
                            .all(|(left, right)| self.is_subtype(left, right));
                }
                if js_subtype(left_identity, right_identity) {
                    return true;
                }
                self.signatures.relation(
                    left,
                    TypeRelation::SubtypeOfDeclaration(right_identity),
                    self.model,
                ) == Truth::Yes
            }
            (ResolvedTypeView::Null, ResolvedTypeView::Null) => true,
            (ResolvedTypeView::Void, ResolvedTypeView::Void) => true,
            (ResolvedTypeView::Function { .. }, ResolvedTypeView::Function { .. })
            | (ResolvedTypeView::Record { .. }, ResolvedTypeView::Record { .. }) => left == right,
            _ => false,
        }
    }

    fn original_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        match (left.view(), right.view()) {
            (
                ResolvedTypeView::Interface {
                    identity: left_identity,
                    ..
                },
                ResolvedTypeView::Interface {
                    identity: right_identity,
                    ..
                },
            ) => {
                left_identity == right_identity
                    || self.signatures.relation(
                        left,
                        TypeRelation::SubtypeOfDeclaration(right_identity),
                        self.model,
                    ) == Truth::Yes
            }
            (
                ResolvedTypeView::TypeParameter { bound, .. }
                | ResolvedTypeView::NonNullTypeParameter { bound, .. },
                _,
            ) => self.original_subtype(bound, right),
            _ => false,
        }
    }

    fn check(&mut self, source: &ResolvedType, target: &ResolvedType, span: &Span, is_check: bool) {
        if self.invalid_test(source, target, is_check) {
            self.diagnostics.push(self.ctx.diagnostic(if is_check {
                "Runtime type checks involving JS interop types are platform-dependent."
            } else {
                "This cast between JS interop and Dart types is not portable."
            }, *span));
        }
    }
}

struct RuntimeChecks<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for RuntimeChecks<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let (expr, target, span, is_check) = match node {
            Expr::Is(node) => (node.expr(), node.dart_type(), node.span(), true),
            Expr::As(node) => (node.expr(), node.dart_type(), node.span(), false),
            _ => return,
        };
        let mut checker = Collector {
            diagnostics: &mut self.diagnostics,
            ctx: self.ctx,
            model: &state.model,
            signatures: &state.signatures,
        };
        checker.check(
            &state.infer(expr),
            &state.model.resolve_type_in(target, &state.type_parameters),
            &span,
            is_check,
        );
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        let checker = Collector {
            diagnostics: &mut self.diagnostics,
            ctx: self.ctx,
            model: &state.model,
            signatures: &state.signatures,
        };
        for catch in statement.catches() {
            if let Some(exception_type) = catch.exception_type() {
                let ty = state
                    .model
                    .resolve_type_in(exception_type, &state.type_parameters);
                if checker.has_interop(&ty, &mut HashSet::new()) == Some(true) {
                    checker.diagnostics.push(checker.ctx.diagnostic("JS interop types cannot be used as catch types.", exception_type.span()));
                }
            }
        }
    }
}

fn paired_invalid(
    collector: &Collector<'_, '_>,
    left: &[ResolvedType],
    right: &[ResolvedType],
    is_check: bool,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .any(|(left, right)| collector.invalid_test(left, right, is_check))
}

fn named_invalid(
    collector: &Collector<'_, '_>,
    left: &[(String, ResolvedType)],
    right: &[(String, ResolvedType)],
    is_check: bool,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .any(|((left_name, left), (right_name, right))| {
                left_name == right_name && collector.invalid_test(left, right, is_check)
            })
}

fn has_annotation(annotations: &[DeclarationIdentity], name: &str) -> bool {
    annotations
        .iter()
        .any(|annotation| annotation.is_sdk("dart:js_interop", name))
}

fn js_type(identity: DeclarationIdentity) -> ResolvedType {
    ResolvedType::new_interface(identity, Vec::new().into(), false, true)
}

fn is_js_sdk(identity: &DeclarationIdentity) -> bool {
    matches!(identity, DeclarationIdentity::Sdk { library, .. } if library.as_str() == "dart:js_interop")
}

fn js_subtype(left: &DeclarationIdentity, right: &DeclarationIdentity) -> bool {
    if !is_js_sdk(left) || !is_js_sdk(right) {
        return false;
    }
    let left = left.name();
    let right = right.name();
    if right == "JSAny" {
        return true;
    }
    if left == right {
        return true;
    }
    if right == "JSObject" {
        return matches!(
            left,
            "JSObject"
                | "JSFunction"
                | "JSExportedDartFunction"
                | "JSBoxedDartObject"
                | "JSArray"
                | "JSPromise"
                | "JSArrayBuffer"
                | "JSDataView"
                | "JSTypedArray"
                | "JSInt8Array"
                | "JSUint8Array"
                | "JSUint8ClampedArray"
                | "JSInt16Array"
                | "JSUint16Array"
                | "JSInt32Array"
                | "JSUint32Array"
                | "JSFloat32Array"
                | "JSFloat64Array"
                | "JSBigInt64Array"
                | "JSBigUint64Array"
        );
    }
    right == "JSTypedArray"
        && matches!(
            left,
            "JSInt8Array"
                | "JSUint8Array"
                | "JSUint8ClampedArray"
                | "JSInt16Array"
                | "JSUint16Array"
                | "JSInt32Array"
                | "JSUint32Array"
                | "JSFloat32Array"
                | "JSFloat64Array"
                | "JSBigInt64Array"
                | "JSBigUint64Array"
        )
}

#[cfg(test)]
mod tests {
    use crate::lint::test_support::diagnostics;
    use std::path::Path;

    use super::*;

    #[test]
    fn catch_diagnostic_covers_only_the_exception_type() {
        let source = "import 'dart:js_interop'; void f() { try {} on JSAny catch (error) {} }";
        let diagnostics = diagnostics(
            InvalidRuntimeCheckWithJsInteropTypes::NAME,
            &InvalidRuntimeCheckWithJsInteropTypes,
            Path::new("/project/lib/test.dart"),
            source,
            false,
        );
        let slices: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(slices, ["JSAny"]);
    }
}
