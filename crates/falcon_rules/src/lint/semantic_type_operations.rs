use std::borrow::Cow;

use falcon_analyze::{AnalyzeContext, ResolvedType, SemanticModel, SignatureIndex};
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
