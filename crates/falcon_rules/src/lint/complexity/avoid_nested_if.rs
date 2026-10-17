//! Flags an `if` statement whose then-branch contains nested `if` statements.
//!
//! Pyramid-shaped nesting is hard to follow; combining conditions with `&&`,
//! using early returns, or extracting a method flattens it. The rule counts the
//! `if` statements within a then-branch subtree and fires when at least two are
//! present (pyramid_lint's default `max_nesting_level` of two), so an `if` whose
//! body nests two further `if`s is reported. Local functions and closures count
//! as part of the then-branch subtree, and closures anywhere in the file are checked.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidNestedIf;

/// pyramid_lint's default `max_nesting_level`: an `if` is flagged when its
/// then-branch subtree contains at least this many `if` statements.
const MAX_NESTING_LEVEL: usize = 2;

impl Rule for AvoidNestedIf {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(NestedIf {
            ctx,
            candidates: Vec::new(),
            active: Vec::new(),
        }))
    }
}

struct Candidate {
    span: Span,
    then_span: Span,
    count: usize,
}

struct NestedIf<'a> {
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<Candidate>,
    active: Vec<usize>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for NestedIf<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.iter().filter(|candidate| candidate.count >= MAX_NESTING_LEVEL)
            .map(|candidate| self.ctx.diagnostic(
                "avoid-nested-if",
                "Avoid nesting if statements. Consider combining conditions or using early returns.",
                candidate.span,
            )).collect()
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::If(n) = node {
            for &index in &self.active {
                let candidate = &mut self.candidates[index];
                if candidate.then_span.start <= n.span.start
                    && n.span.end <= candidate.then_span.end
                    && candidate.then_span != n.span
                {
                    candidate.count += 1;
                }
            }
            self.active.push(self.candidates.len());
            self.candidates.push(Candidate {
                span: n.span, then_span: *n.then_branch.span(), count: 0,
            });
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: &Stmt) {
        if matches!(node, Stmt::If(_)) {
            self.active.pop();
        }
    }
}
}
