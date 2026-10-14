//! Reject platform-dependent runtime checks involving JavaScript interop types.

use std::collections::HashSet;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct InvalidRuntimeCheckWithJsInteropTypes;

impl Rule for InvalidRuntimeCheckWithJsInteropTypes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut visitor = RuntimeChecks {
            ctx,
            diagnostics: Vec::new(),
        };
        visit_program(&mut visitor, program, state);
        visitor.diagnostics
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

struct Collector<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
}

impl Collector<'_> {
    fn normalize(&self, ty: &ResolvedType) -> Normalized {
        self.normalize_inner(ty, &mut HashSet::new())
    }

    fn normalize_inner(
        &self,
        ty: &ResolvedType,
        visiting: &mut HashSet<DeclarationIdentity>,
    ) -> Normalized {
        match ty {
            ResolvedType::Unknown => Normalized {
                ty: ResolvedType::Unknown,
                interop: Interop::Unknown,
            },
            ResolvedType::TypeParameter { bound, .. } => self.normalize_inner(bound, visiting),
            ResolvedType::Interface {
                identity,
                arguments,
                extension_type,
                ..
            } => {
                if let DeclarationIdentity::Sdk {
                    library: "dart:js_interop",
                    name,
                } = identity
                {
                    return Normalized {
                        ty: js_type(name),
                        interop: Interop::Sdk,
                    };
                }
                if !visiting.insert(identity.clone()) {
                    return Normalized {
                        ty: ResolvedType::Unknown,
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
                        ty: js_type("JSObject"),
                        interop: Interop::User(identity.clone()),
                    }
                } else if *extension_type || representation.is_some() {
                    representation.map_or(
                        Normalized {
                            ty: ResolvedType::Unknown,
                            interop: Interop::Unknown,
                        },
                        |representation| {
                            let normalized = self.normalize_inner(representation, visiting);
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
                        ty: ResolvedType::Interface {
                            identity: identity.clone(),
                            arguments: arguments
                                .iter()
                                .map(|argument| self.normalize_inner(argument, visiting).ty)
                                .collect(),
                            nullable: false,
                            extension_type: false,
                        },
                        interop: Interop::None,
                    }
                };
                visiting.remove(identity);
                result
            }
            ResolvedType::Function {
                return_type,
                positional,
                named,
                ..
            } => Normalized {
                ty: ResolvedType::Function {
                    return_type: Box::new(self.normalize_inner(return_type, visiting).ty),
                    positional: positional
                        .iter()
                        .map(|ty| self.normalize_inner(ty, visiting).ty)
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, ty)| (name.clone(), self.normalize_inner(ty, visiting).ty))
                        .collect(),
                    nullable: false,
                },
                interop: Interop::None,
            },
            ResolvedType::Record {
                positional, named, ..
            } => Normalized {
                ty: ResolvedType::Record {
                    positional: positional
                        .iter()
                        .map(|ty| self.normalize_inner(ty, visiting).ty)
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, ty)| (name.clone(), self.normalize_inner(ty, visiting).ty))
                        .collect(),
                    nullable: false,
                },
                interop: Interop::None,
            },
            other => Normalized {
                ty: other.with_nullable(false),
                interop: if matches!(other, ResolvedType::Dynamic) {
                    Interop::Unknown
                } else {
                    Interop::None
                },
            },
        }
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
                if !left_subtype && !matches!(right_normalized.ty, ResolvedType::Dynamic) {
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
                && !matches!(left_normalized.ty, ResolvedType::Dynamic)
                && !matches!(right_normalized.ty, ResolvedType::Dynamic)
            {
                return true;
            }
        }
        self.invalid_nested(left, right, is_check)
    }

    fn invalid_nested(&self, left: &ResolvedType, right: &ResolvedType, is_check: bool) -> bool {
        match (left, right) {
            (
                ResolvedType::Interface {
                    arguments: left, ..
                },
                ResolvedType::Interface {
                    arguments: right, ..
                },
            ) if left.len() == right.len() => left
                .iter()
                .zip(right)
                .any(|(left, right)| self.invalid_test(left, right, is_check)),
            (
                ResolvedType::Function {
                    return_type: left_return,
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Function {
                    return_type: right_return,
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                self.invalid_test(left_return, right_return, is_check)
                    || paired_invalid(self, right_positional, left_positional, is_check)
                    || named_invalid(self, right_named, left_named, is_check)
            }
            (
                ResolvedType::Record {
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Record {
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                paired_invalid(self, left_positional, right_positional, is_check)
                    || named_invalid(self, left_named, right_named, is_check)
            }
            (ResolvedType::TypeParameter { bound, .. }, right) => {
                self.invalid_test(bound, right, is_check)
            }
            (left, ResolvedType::TypeParameter { bound, .. }) => {
                self.invalid_test(left, bound, is_check)
            }
            _ => false,
        }
    }

    fn is_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        if matches!(left, ResolvedType::Never | ResolvedType::Dynamic)
            || matches!(right, ResolvedType::Dynamic)
        {
            return true;
        }
        if let ResolvedType::Interface { identity, .. } = right
            && *identity == DeclarationIdentity::OBJECT
        {
            return true;
        }
        match (left, right) {
            (
                ResolvedType::Interface {
                    identity: left_identity,
                    arguments: left_arguments,
                    ..
                },
                ResolvedType::Interface {
                    identity: right_identity,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                if left_identity == right_identity {
                    return left_arguments.len() == right_arguments.len()
                        && left_arguments
                            .iter()
                            .zip(right_arguments)
                            .all(|(left, right)| self.is_subtype(left, right));
                }
                if js_subtype(left_identity, right_identity) {
                    return true;
                }
                self.signatures
                    .is_subtype_of(left, right_identity, self.model)
                    == Truth::Yes
            }
            (ResolvedType::Null, ResolvedType::Null) => true,
            (ResolvedType::Void, ResolvedType::Void) => true,
            (ResolvedType::Function { .. }, ResolvedType::Function { .. })
            | (ResolvedType::Record { .. }, ResolvedType::Record { .. }) => left == right,
            _ => false,
        }
    }

    fn original_subtype(&self, left: &ResolvedType, right: &ResolvedType) -> bool {
        match (left, right) {
            (
                ResolvedType::Interface {
                    identity: left_identity,
                    ..
                },
                ResolvedType::Interface {
                    identity: right_identity,
                    ..
                },
            ) => {
                left_identity == right_identity
                    || self
                        .signatures
                        .is_subtype_of(left, right_identity, self.model)
                        == Truth::Yes
            }
            (ResolvedType::TypeParameter { bound, .. }, right) => {
                self.original_subtype(bound, right)
            }
            _ => false,
        }
    }

    fn check(&mut self, source: &ResolvedType, target: &ResolvedType, span: &Span, is_check: bool) {
        if self.invalid_test(source, target, is_check) {
            self.diagnostics.push(self.ctx.diagnostic(
                "invalid-runtime-check-with-js-interop-types",
                if is_check {
                    "Runtime type checks involving JS interop types are platform-dependent."
                } else {
                    "This cast between JS interop and Dart types is not portable."
                },
                *span,
            ));
        }
    }
}

struct RuntimeChecks<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for RuntimeChecks<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let (expr, target, span, is_check) = match node {
            Expr::Is {
                expr,
                dart_type,
                span,
                ..
            } => (expr, dart_type, span, true),
            Expr::As {
                expr,
                dart_type,
                span,
            } => (expr, dart_type, span, false),
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
            span,
            is_check,
        );
    }

    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        let checker = Collector {
            diagnostics: &mut self.diagnostics,
            ctx: self.ctx,
            model: &state.model,
            signatures: &state.signatures,
        };
        for catch in &statement.catches {
            if let Some(exception_type) = &catch.exception_type {
                let ty = state
                    .model
                    .resolve_type_in(exception_type, &state.type_parameters);
                if !matches!(
                    checker.normalize(&ty).interop,
                    Interop::None | Interop::Unknown
                ) {
                    checker.diagnostics.push(checker.ctx.diagnostic(
                        "invalid-runtime-check-with-js-interop-types",
                        "JS interop types cannot be used as catch types.",
                        catch.span,
                    ));
                }
            }
        }
    }
}

fn paired_invalid(
    collector: &Collector<'_>,
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
    collector: &Collector<'_>,
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

fn js_type(name: &'static str) -> ResolvedType {
    ResolvedType::Interface {
        identity: DeclarationIdentity::Sdk {
            library: "dart:js_interop",
            name,
        },
        arguments: Vec::new(),
        nullable: false,
        extension_type: true,
    }
}

fn is_js_sdk(identity: &DeclarationIdentity) -> bool {
    matches!(
        identity,
        DeclarationIdentity::Sdk {
            library: "dart:js_interop",
            ..
        }
    )
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
