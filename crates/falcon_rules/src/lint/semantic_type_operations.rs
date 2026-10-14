use std::borrow::Cow;

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex, Truth,
    TypeIndex,
};
use falcon_syntax::ast::*;

use super::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

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

pub(crate) struct TypeOperationAnalysis<'a> {
    pub(crate) model: SemanticModel<'a>,
    pub(crate) signatures: Cow<'a, SignatureIndex>,
    pub(crate) operations: Vec<TypeOperation>,
}

pub(crate) fn collect<'a>(
    program: &Program,
    ctx: &'a AnalyzeContext<'a>,
) -> Option<TypeOperationAnalysis<'a>> {
    let state = SemanticState::new(program, ctx)?;
    let mut collector = Collector {
        operations: Vec::new(),
    };
    let state = visit_program(&mut collector, program, state);
    Some(TypeOperationAnalysis {
        model: state.model,
        signatures: state.signatures,
        operations: collector.operations,
    })
}

struct Collector {
    operations: Vec<TypeOperation>,
}

impl SemanticRuleVisitor for Collector {
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
        self.operations.push(TypeOperation {
            kind,
            operand: state.infer(expr),
            target: state
                .model
                .resolve_type_in(dart_type, &state.type_parameters),
            span: *span,
        });
    }
}

/// Three-valued nullability of a resolved type, mirroring the analyzer's
/// `TypeSystem.isNullable` (`Yes`) and `isNonNullable` (`No`).
///
/// `Unknown` covers potentially nullable types that are neither (an unbounded
/// type parameter, an extension type) as well as anything falcon cannot prove:
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
        ResolvedType::Never => Truth::No,
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
            arguments,
            nullable,
            extension_type,
        } => {
            if *nullable {
                return Truth::Yes;
            }
            if *extension_type {
                return Truth::Unknown;
            }
            match identity {
                DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Null",
                } => Truth::Yes,
                DeclarationIdentity::Sdk {
                    library: "dart:async",
                    name: "FutureOr",
                } => arguments.first().map_or(Truth::Unknown, |argument| {
                    nullability(argument, signatures, types)
                }),
                DeclarationIdentity::Sdk { .. } | DeclarationIdentity::Package { .. } => Truth::No,
                // A project name may be a (possibly nullable) type alias; only a
                // known class-like declaration proves non-nullability.
                DeclarationIdentity::Project { name, .. } => {
                    let class_like = signatures
                        .declaration(identity)
                        .is_some_and(|facts| facts.extension_representation.is_none())
                        || types.is_some_and(|types| {
                            matches!(
                                types.type_kind(name),
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
