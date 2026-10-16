//! Explicitly tear off callable objects' call methods.
//!
//! Adopted from the official `implicit_call_tearoffs` lint.
//! Reports each `ImplicitCallReference` the analyzer inserts. The coercion
//! applies to the outermost expression of a conditional, `??`, or cascade (and to a type
//! instantiation, `c<int>`), so their operands are not reported again.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::{BinaryOp, Expr, Span};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct ImplicitCallTearoffs;

impl Rule for ImplicitCallTearoffs {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            coerced: HashSet::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Operands whose value flows into an already coerced expression.
    coerced: HashSet<Span>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let inside_coerced = self.coerced.contains(node.span());
        if !inside_coerced && !state.implicit_call_tearoff(node) {
            return;
        }
        match node {
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => {
                self.coerced.insert(*then_expr.span());
                self.coerced.insert(*else_expr.span());
            }
            Expr::Binary {
                op: BinaryOp::NullCoalesce,
                left,
                right,
                ..
            } => {
                self.coerced.insert(*left.span());
                self.coerced.insert(*right.span());
            }
            Expr::Cascade { object, .. } => {
                self.coerced.insert(*object.span());
            }
            Expr::GenericInstantiation { target, .. } => {
                self.coerced.insert(*target.span());
            }
            _ => {}
        }
        if inside_coerced {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "implicit-call-tearoffs",
            "Explicitly tear off the 'call' method.",
            *node.span(),
        ));
    }
}
