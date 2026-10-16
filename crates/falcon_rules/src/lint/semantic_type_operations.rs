use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{DeclarationIdentity, ResolvedType, SignatureIndex, Truth, TypeIndex};
use falcon_syntax::ast::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;

pub(crate) enum TypeOperationKind {
    Is { negated: bool },
    As,
}

pub(crate) struct TypeOperation {
    pub(crate) kind: TypeOperationKind,
    pub(crate) operand: ResolvedType,
    pub(crate) target: ResolvedType,
    pub(crate) span: Span,
}

pub(crate) struct TypeOperations<F> {
    pub(crate) report: F,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl<F> SemanticRuleVisitor for TypeOperations<F>
where
    F: FnMut(TypeOperation, &SemanticState<'_>) -> Option<Diagnostic>,
{
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let (kind, expr, dart_type, span) = match node {
            Expr::Is {
                expr,
                dart_type,
                negated,
                span,
            } => (
                TypeOperationKind::Is { negated: *negated },
                expr,
                dart_type,
                span,
            ),
            Expr::As {
                expr,
                dart_type,
                span,
            } => (TypeOperationKind::As, expr, dart_type, span),
            _ => return,
        };
        let operation = TypeOperation {
            kind,
            operand: state.infer(expr),
            target: state
                .model
                .resolve_type_in(dart_type, &state.type_parameters),
            span: *span,
        };
        if let Some(diagnostic) = (self.report)(operation, state) {
            self.diagnostics.push(diagnostic);
        }
    }
}

/// Three-valued nullability of a resolved type, mirroring the analyzer's
/// `TypeSystem.isNullable` (`Yes`) and `isNonNullable` (`No`).
///
/// `Unknown` covers potentially nullable types that are neither (an unbounded
/// type parameter, an extension type without superinterfaces) as well as anything falcon cannot prove:
/// `dynamic`, `void`, unresolved types, and project names that may be type
/// aliases. Callers that need the analyzer's `isNullable(dynamic)` must check
/// [`ResolvedType::Dynamic`] themselves.
pub(crate) fn nullability(
    ty: &ResolvedType,
    signatures: &SignatureIndex,
    types: Option<&TypeIndex>,
) -> Truth {
    match ty {
        ResolvedType::Null => Truth::Yes,
        ResolvedType::Never | ResolvedType::NonNullTypeParameter { .. } => Truth::No,
        ResolvedType::Unknown | ResolvedType::Dynamic | ResolvedType::Void => Truth::Unknown,
        ResolvedType::Function { nullable, .. } | ResolvedType::Record { nullable, .. } => {
            if *nullable {
                Truth::Yes
            } else {
                Truth::No
            }
        }
        ResolvedType::TypeParameter {
            bound, nullable, ..
        } => {
            if *nullable {
                Truth::Yes
            } else if nullability(bound, signatures, types) == Truth::No {
                Truth::No
            } else {
                Truth::Unknown
            }
        }
        ResolvedType::Interface {
            identity,
            nullable,
            extension_type,
            ..
        } => {
            if *nullable {
                return Truth::Yes;
            }
            if *extension_type {
                // A non-nullable extension type with a superinterface is
                // non-nullable even when its representation admits null.
                return if signatures.supertype_matches(identity, &|other| other != identity)
                    == Truth::Yes
                {
                    Truth::No
                } else {
                    Truth::Unknown
                };
            }
            match identity {
                DeclarationIdentity::Sdk { library, name }
                    if library.as_str() == "dart:core" && name.as_str() == "Null" =>
                {
                    Truth::Yes
                }
                DeclarationIdentity::Sdk { library, name }
                    if library.as_str() == "dart:async" && name.as_str() == "FutureOr" =>
                {
                    ty.type_argument(0).map_or(Truth::Unknown, |argument| {
                        nullability(&argument, signatures, types)
                    })
                }
                DeclarationIdentity::Sdk { .. } | DeclarationIdentity::Package { .. } => Truth::No,
                // A project name may be a (possibly nullable) type alias; only a
                // known class-like declaration proves non-nullability.
                DeclarationIdentity::Project { .. } => {
                    let class_like = signatures
                        .declaration(identity)
                        .is_some_and(|facts| facts.extension_representation.is_none())
                        || types.is_some_and(|types| {
                            matches!(
                                types.type_kind_identity(identity),
                                Some(TypeKind::Mixin | TypeKind::Enum)
                            )
                        });
                    if class_like {
                        Truth::No
                    } else {
                        Truth::Unknown
                    }
                }
            }
        }
    }
}
