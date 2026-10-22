//! Explicitly tear off callable objects' call methods.
//!
//! Adopted from the official `implicit_call_tearoffs` lint.
//! Reports each `ImplicitCallReference` the analyzer inserts. The coercion
//! applies to the outermost expression of a conditional, `??`, or cascade (and to a type
//! instantiation, `c<int>`), so their operands are not reported again.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Program;
use falcon_syntax::cst::{BinaryOp, Expr, Span};

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct ImplicitCallTearoffs;

impl Rule for ImplicitCallTearoffs {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        let inside_coerced = self.coerced.contains(&node.span());
        if !inside_coerced && !state.implicit_call_tearoff(node) {
            return;
        }
        match node {
            Expr::Conditional(expr) => {
                self.coerced.insert(expr.then_expr().span());
                self.coerced.insert(expr.else_expr().span());
            }
            Expr::Binary(expr) if expr.op() == BinaryOp::NullCoalesce => {
                self.coerced.insert(expr.left().span());
                self.coerced.insert(expr.right().span());
            }
            Expr::Cascade(expr) => {
                self.coerced.insert(expr.object().span());
            }
            Expr::GenericInstantiation(expr) => {
                self.coerced.insert(expr.target().span());
            }
            _ => {}
        }
        if inside_coerced {
            return;
        }
        self.diags.push(self.ctx.diagnostic("Explicitly tear off the `call` method.", node.span()));
    }
}
