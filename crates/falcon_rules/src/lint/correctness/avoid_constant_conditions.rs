//! Flags statically constant binary branch, loop, collection, and expression conditions.
//! Replace the condition with the reachable branch or remove the unreachable code.
//! Expressions containing runtime allocations are conservatively left unchanged.

use crate::lint::support::constants::constant_value;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ConstantValue, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct AvoidConstantConditions;

impl Rule for AvoidConstantConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
            conditions: HashSet::new(),
        }))
    }
}


struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    conditions: HashSet<Span>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, expression: Expr<'a>, _state: &SemanticState<'a>) {
        self.conditions.insert(expression.span());
    }

    fn condition(&mut self, condition: IfCondition<'a>, state: &SemanticState<'a>) {
        match condition {
            IfCondition::Expr(expression) => self.check(expression, state),
            IfCondition::Case(pattern) => {
                if let Some(guard) = pattern.guard() {
                    self.check(guard, state);
                }
            }
        }
    }

    fn element(&mut self, element: CollectionElement<'a>, state: &SemanticState<'a>) {
        match element {
            CollectionElement::If(node) => {
                self.condition(node.condition(), state);
                self.element(node.then_elem(), state);
                if let Some(otherwise) = node.else_elem() {
                    self.element(otherwise, state);
                }
            }
            CollectionElement::For(node) => {
                if let Some(condition) = node.header().condition() {
                    self.check(condition, state);
                }
                self.element(node.element(), state);
            }
            _ => {}
        }
    }

    fn map_element(&mut self, element: MapElement<'a>, state: &SemanticState<'a>) {
        match element {
            MapElement::If(node) => {
                self.condition(node.condition(), state);
                self.map_element(node.then_entry(), state);
                if let Some(otherwise) = node.else_entry() {
                    self.map_element(otherwise, state);
                }
            }
            MapElement::For(node) => {
                if let Some(condition) = node.header().condition() {
                    self.check(condition, state);
                }
                self.map_element(node.entry(), state);
            }
            _ => {}
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::If(node) => self.condition(node.condition(), state),
            Stmt::While(node) => self.check(node.condition(), state),
            Stmt::DoWhile(node) => self.check(node.condition(), state),
            Stmt::For(node) => {
                if let Some(condition) = node.header().condition() {
                    self.check(condition, state);
                }
            }
            Stmt::Switch(node) => {
                for label in node.cases().flat_map(|case| case.cases()) {
                    if let SwitchCaseKind::Pattern(label) = label
                        && let Some(guard) = label.guard()
                    {
                        self.check(guard, state);
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if self.conditions.remove(&node.span())
            && matches!(node.unparenthesized(), Expr::Binary(_))
            && matches!(
                constant_value(node, state, self.ctx),
                Some(ConstantValue::Bool(_))
            )
        {
            self.diagnostics.push(self.ctx.diagnostic("Condition is statically constant; keep only the reachable branch or remove the unreachable code.", node.span()));
        }
        match node {
            Expr::Conditional(node) => self.check(node.condition(), state),
            Expr::List(node) => {
                for element in node.elements() {
                    self.element(element, state);
                }
            }
            Expr::Set(node) => {
                for element in node.elements() {
                    self.element(element, state);
                }
            }
            Expr::Map(node) => {
                for element in node.elements() {
                    self.map_element(element, state);
                }
            }
            Expr::Switch(node) => {
                for arm in node.arms() {
                    if let Some(guard) = arm.guard() {
                        self.check(guard, state);
                    }
                }
            }
            _ => {}
        }
    }
}
