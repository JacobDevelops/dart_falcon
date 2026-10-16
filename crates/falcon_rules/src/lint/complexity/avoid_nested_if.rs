//! Flags an `if` statement whose then-branch contains nested `if` statements.
//!
//! Pyramid-shaped nesting is hard to follow; combining conditions with `&&`,
//! using early returns, or extracting a method flattens it. The rule counts the
//! `if` statements within a then-branch subtree and fires when at least two are
//! present (pyramid_lint's default `max_nesting_level` of two), so an `if` whose
//! body nests two further `if`s is reported. Local functions and closures count
//! as part of the then-branch subtree, and closures anywhere in the file are checked.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct AvoidNestedIf;

/// pyramid_lint's default `max_nesting_level`: an `if` is flagged when its
/// then-branch subtree contains at least this many `if` statements.
const MAX_NESTING_LEVEL: usize = 2;

impl Rule for AvoidNestedIf {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        NestedIf {
            diags: &mut diags,
            ctx,
        }
        .visit_program(program);
        RulePreparation::Diagnostics(diags)
    }
}

struct IfCount(usize);

impl Visitor for IfCount {
    fn visit_stmt(&mut self, node: &Stmt) {
        self.0 += usize::from(matches!(node, Stmt::If(_)));
        walk_stmt(self, node);
    }
}

struct NestedIf<'a, 'ctx> {
    diags: &'a mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for NestedIf<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(n) = node {
            let mut count = IfCount(0);
            // Count children, not a then-branch that is itself an if.
            walk_stmt(&mut count, &n.then_branch);
            if count.0 >= MAX_NESTING_LEVEL {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-nested-if",
                    "Avoid nesting if statements. Consider combining conditions or using early returns.",
                    n.span,
                ));
            }
        }
        walk_stmt(self, node);
    }
}
