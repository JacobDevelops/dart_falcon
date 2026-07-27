//! Prefers `contains` to `indexOf` comparisons used only as membership tests.

use falcon_analyze::{AnalyzeContext, Rule, parse_int};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct PreferContains;

impl Rule for PreferContains {
    fn name(&self) -> &'static str {
        "prefer-contains"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl SemanticRuleVisitor for Collector {
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
        let invocation = index_of_invocation(left)
            .filter(|_| comparison_matches(op, right, false))
            .or_else(|| index_of_invocation(right).filter(|_| comparison_matches(op, left, true)));
        let Some(invocation) = invocation else {
            return;
        };
        let receiver = state.infer(invocation.receiver);
        let is_string = receiver.interface("dart:core", "String");
        let is_list = state
            .signatures
            .instantiated_supertype(&receiver, "dart:core", "List", &state.model)
            .is_some();
        if invocation.argument_count == 1 && (is_string || is_list) {
            self.diags.push(Diagnostic::new(
                "prefer-contains",
                Severity::Warning,
                "Use 'contains' instead of comparing the result of 'indexOf'.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

struct IndexOfInvocation<'a> {
    receiver: &'a Expr,
    argument_count: usize,
}

fn index_of_invocation(expr: &Expr) -> Option<IndexOfInvocation<'_>> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let Expr::Field {
        object,
        field,
        is_null_safe: false,
        ..
    } = callee.as_ref()
    else {
        return None;
    };
    if field.name != "indexOf" || !args.named.is_empty() || args.positional.is_empty() {
        return None;
    }
    Some(IndexOfInvocation {
        receiver: object,
        argument_count: args.positional.len(),
    })
}

fn int_value(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::IntLit { value, .. } => parse_int(value),
        Expr::Unary {
            op: UnaryOp::Minus,
            operand,
            ..
        } => match operand.as_ref() {
            Expr::IntLit { value, .. } => parse_int(value).and_then(i128::checked_neg),
            _ => None,
        },
        _ => None,
    }
}

fn comparison_matches(op: &BinaryOp, other: &Expr, swapped: bool) -> bool {
    let Some(value) = int_value(other) else {
        return false;
    };
    matches!(
        (op, swapped, value),
        (BinaryOp::EqEq | BinaryOp::NotEq, _, -1)
            | (BinaryOp::Gt, false, -1)
            | (BinaryOp::Lt, true, -1)
            | (BinaryOp::GtEq, false, 0)
            | (BinaryOp::LtEq, true, 0)
            | (BinaryOp::Lt, false, 0)
            | (BinaryOp::Gt, true, 0)
            | (BinaryOp::LtEq, false, -1)
            | (BinaryOp::GtEq, true, -1)
    )
}
