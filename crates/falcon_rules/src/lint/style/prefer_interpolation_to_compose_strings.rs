//! Prefers string interpolation to composing strings with `+`.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct PreferInterpolationToComposeStrings;

impl Rule for PreferInterpolationToComposeStrings {
    fn name(&self) -> &'static str {
        "prefer-interpolation-to-compose-strings"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            nested_chain_spans: HashSet::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
    nested_chain_spans: HashSet<(usize, usize)>,
}

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        if self.nested_chain_spans.contains(&(span.start, span.end)) {
            return;
        }
        if !core_string(&state.infer(left)) || !core_string(&state.infer(right)) {
            return;
        }
        let mut composition = Composition::default();
        composition.visit_expr(node);
        if composition.has_string_literal
            && composition.has_non_literal_operand
            && !composition.has_raw_string
        {
            let mut nested = NestedAdditions(&mut self.nested_chain_spans);
            nested.visit_expr(left);
            nested.visit_expr(right);
            self.diags.push(Diagnostic::new(
                "prefer-interpolation-to-compose-strings",
                Severity::Warning,
                "Use string interpolation to compose strings and values.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

struct NestedAdditions<'a>(&'a mut HashSet<(usize, usize)>);

impl Visitor for NestedAdditions<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Binary {
            op: BinaryOp::Add,
            span,
            ..
        } = node
        {
            self.0.insert((span.start, span.end));
            walk_expr(self, node);
        }
    }
}

fn core_string(ty: &ResolvedType) -> bool {
    ty.interface("dart:core", "String") && !ty.nullable()
}

#[derive(Default)]
struct Composition {
    has_string_literal: bool,
    has_non_literal_operand: bool,
    has_raw_string: bool,
}

impl Visitor for Composition {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::StringLit(string) => {
                self.has_string_literal = true;
                self.has_non_literal_operand |= !string.interpolations.is_empty();
                self.has_raw_string |= matches!(string.raw.as_bytes().first(), Some(b'r' | b'R'));
            }
            Expr::Binary {
                op: BinaryOp::Add, ..
            } => walk_expr(self, node),
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::SymbolLit { .. }
            | Expr::Ident(_)
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::List { .. }
            | Expr::Set { .. }
            | Expr::Map { .. }
            | Expr::Record { .. }
            | Expr::New { .. }
            | Expr::Call { .. }
            | Expr::Field { .. }
            | Expr::Index { .. }
            | Expr::Assign { .. }
            | Expr::Conditional { .. }
            | Expr::Unary { .. }
            | Expr::PostfixIncDec { .. }
            | Expr::Is { .. }
            | Expr::As { .. }
            | Expr::NullAssert { .. }
            | Expr::Cascade { .. }
            | Expr::FuncExpr { .. }
            | Expr::DotShorthand { .. }
            | Expr::Await { .. }
            | Expr::Throw { .. }
            | Expr::Switch { .. }
            | Expr::GenericInstantiation { .. }
            | Expr::Error { .. }
            | Expr::Binary { .. } => self.has_non_literal_operand = true,
        }
    }
}
