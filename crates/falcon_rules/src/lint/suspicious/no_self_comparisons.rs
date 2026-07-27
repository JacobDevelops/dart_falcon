//! Flags proven self-comparisons of side-effect-free scalar values.
//!
//! Getter reads, index operations, calls, and user-defined operator receivers are
//! intentionally excluded: evaluating the same syntax twice need not produce the
//! same value, and equality/ordering may be overloaded.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct NoSelfComparisons;

impl Rule for NoSelfComparisons {
    fn name(&self) -> &'static str {
        "no-self-comparisons"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    file: String,
    source: &'a str,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        if !is_comparison(op)
            || !proven_scalar_value(left, state)
            || !proven_scalar_value(right, state)
        {
            return;
        }
        let (Some(left_text), Some(right_text)) = (
            normalized(self.source, left.span()),
            normalized(self.source, right.span()),
        ) else {
            return;
        };
        if left_text != right_text {
            return;
        }
        self.diags.push(Diagnostic::new(
            "no-self-comparisons",
            Severity::Warning,
            "Both operands of this comparison are identical.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn is_comparison(op: &BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
    )
}

fn proven_scalar_value(expression: &Expr, state: &SemanticState<'_>) -> bool {
    let syntactically_stable = match expression {
        Expr::Ident(_)
        | Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. } => true,
        Expr::StringLit(string) => string.interpolations.is_empty(),
        // `++x`/`--x` mutate, so the two sides are not the same value.
        Expr::Unary { op, operand, .. } => {
            !matches!(op, UnaryOp::PlusPlus | UnaryOp::MinusMinus)
                && matches!(
                    operand.as_ref(),
                    Expr::Ident(_) | Expr::IntLit { .. } | Expr::DoubleLit { .. }
                )
        }
        _ => false,
    };
    syntactically_stable && scalar_type(&state.infer(expression))
}

fn scalar_type(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Null => true,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            ..
        } => {
            library == "dart:core"
                && matches!(name.as_str(), "bool" | "int" | "double" | "num" | "String")
        }
        _ => false,
    }
}

/// `None` when the span is out of bounds or off a UTF-8 boundary, so a bad span
/// cannot panic or make two different operands normalize alike.
fn normalized(source: &str, span: &Span) -> Option<String> {
    Some(
        source
            .get(span.start..span.end)?
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect(),
    )
}
