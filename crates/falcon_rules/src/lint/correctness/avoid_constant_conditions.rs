//! Flags statically constant binary branch, loop, collection, and expression conditions.
//! Replace the condition with the reachable branch or remove the unreachable code.
//! Expressions containing runtime allocations are conservatively left unchanged.

use crate::lint::const_eval::{ConstContexts, constant_in_context, deferred_prefixes};
use falcon_analyze::constant_value::evaluate_stable_constant_with_local_values;
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

pub(crate) fn constant_value(
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> Option<ConstantValue> {
    // An extension's implicit receiver is its on-type, not the extension's
    // lexical member namespace. In particular, unnamed extension members have
    // no declaration identity for the constant evaluator to inspect.
    if state.enclosing_type.is_some()
        && let Some(extension) = expression
            .syntax()
            .ancestors()
            .find_map(ExtensionDecl::cast)
    {
        struct InstanceReferences<'a> {
            extension: ExtensionDecl<'a>,
            found: bool,
        }
        impl<'a> falcon_syntax::cst::visit::Visitor<'a> for InstanceReferences<'_> {
            fn visit_expr(&mut self, node: Expr<'a>) {
                if let Expr::Ident(identifier) = node {
                    self.found |= self.extension.members().any(|member| match member {
                        ClassMember::Getter(getter) => {
                            !getter.is_static() && getter.name().name() == identifier.name()
                        }
                        ClassMember::Method(method) => {
                            !method.is_static() && method.name().name() == identifier.name()
                        }
                        _ => false,
                    });
                }
                if !self.found {
                    falcon_syntax::cst::visit::walk_expr(self, node);
                }
            }
        }
        let mut references = InstanceReferences {
            extension,
            found: false,
        };
        falcon_syntax::cst::visit::Visitor::visit_expr(&mut references, expression);
        if references.found {
            return None;
        }
    }
    if has_runtime_allocation(expression, state, ctx.parsed.program()) {
        return None;
    }
    if !constant_in_context(expression, &deferred_prefixes(ctx.parsed.program()), state) {
        return None;
    }
    evaluate_stable_constant_with_local_values(
        expression,
        state.constant_owner.as_ref(),
        &state.visible_local_constants(),
        &state.model,
        &state.signatures,
    )
}

fn has_runtime_allocation(
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    program: Program<'_>,
) -> bool {
    struct Allocations<'s, 'a> {
        state: &'s SemanticState<'a>,
        found: bool,
    }
    impl<'a> falcon_syntax::cst::visit::Visitor<'a> for Allocations<'_, 'a> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            let allocation = match node {
                Expr::List(node) => Some(node.is_const()),
                Expr::Map(node) => Some(node.is_const()),
                Expr::Set(node) => Some(node.is_const()),
                Expr::Record(node) => Some(node.is_const()),
                Expr::New(node) => Some(node.is_const()),
                Expr::Call(call)
                    if matches!(call.callee().unparenthesized(),
                        Expr::DotShorthand(node) if node.is_const()) =>
                {
                    Some(true)
                }
                Expr::Call(call)
                    if falcon_analyze::resolve_call_constructor(
                        call.callee(),
                        &self.state.model,
                        true,
                    )
                    .is_some() =>
                {
                    Some(false)
                }
                _ => None,
            };
            match allocation {
                Some(false) => self.found = true,
                // Explicit const allocations also make their contents constant.
                Some(true) => {}
                None if !self.found => falcon_syntax::cst::visit::walk_expr(self, node),
                None => {}
            }
        }
    }
    let mut allocations = Allocations {
        state,
        found: false,
    };
    falcon_syntax::cst::visit::Visitor::visit_expr(&mut allocations, expression);
    allocations.found && !ConstContexts::of(program).contains(&expression.span())
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
            self.diagnostics.push(self.ctx.diagnostic("avoid-constant-conditions", "Condition is statically constant; keep only the reachable branch or remove the unreachable code.", node.span()));
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
