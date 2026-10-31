//! Prefer `AsyncValue.when` to manual loading, error, and data dispatch.
//!
//! Reports once at the head of an if/else-if statement, collection-if element,
//! or conditional-expression chain checking at least two distinct states of the
//! same resolved Riverpod value, including bounded type parameters.
//! Conditions must be individual state getters or type tests, optionally negated;
//! compound conditions, isolated checks, and switch patterns are left alone.
//! No fix is offered because `when` has different refresh/loading semantics.

use std::collections::HashMap;

use crate::lint::riverpod;
use crate::lint::support::element_identity::same_resolved_element;
use falcon_analyze::{
    AnalyzeContext, ResolvedTypeView, Rule, RulePreparation, SemanticRuleVisitor, SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{
    CollectionElement, Expr, IfCondition, IfElement, MapElement, MapIfElement, NodeId, NodeKind, Program,
    Span, Stmt, UnaryOp,
};

#[derive(Default)]
pub struct PreferAsyncValueWhen;

impl Rule for PreferAsyncValueWhen {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            chains: Vec::new(),
            continuations: HashMap::new(),
            conditions: HashMap::new(),
        }))
    }
}

struct Chain<'a> {
    span: Span,
    receiver: Option<Expr<'a>>,
    states: u8,
    valid: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    chains: Vec<Chain<'a>>,
    continuations: HashMap<NodeId, usize>,
    conditions: HashMap<NodeId, usize>,
}

impl<'a> Collector<'a> {
    fn chain(&mut self, id: NodeId, span: Span) -> usize {
        self.continuations.remove(&id).unwrap_or_else(|| {
            let index = self.chains.len();
            self.chains.push(Chain {
                span,
                receiver: None,
                states: 0,
                valid: true,
            });
            index
        })
    }

    fn state_check(
        &self,
        mut expression: Expr<'a>,
        state: &SemanticState<'a>,
    ) -> Option<(Expr<'a>, u8)> {
        expression = expression.unparenthesized();
        while let Expr::Unary(unary) = expression {
            if unary.op() != UnaryOp::Bang {
                return None;
            }
            expression = unary.operand().unparenthesized();
        }
        let (receiver, flag) = match expression {
            Expr::Field(field) => {
                let flag = match field.field().name() {
                    "isLoading" => 1,
                    "hasError" => 2,
                    "hasValue" => 4,
                    _ => return None,
                };
                (field.object(), flag)
            }
            Expr::Is(test) => {
                let target = state.model.resolve_type_in(test.dart_type(), &state.type_parameters);
                let flag = if riverpod::is_exact_type(self.ctx, &target, &["AsyncLoading"]) {
                    1
                } else if riverpod::is_exact_type(self.ctx, &target, &["AsyncError"]) {
                    2
                } else if riverpod::is_exact_type(self.ctx, &target, &["AsyncData"]) {
                    4
                } else {
                    return None;
                };
                (test.expr(), flag)
            }
            _ => return None,
        };
        let inferred = state.infer(receiver);
        let mut bound = &inferred;
        while let ResolvedTypeView::TypeParameter { bound: next, .. }
        | ResolvedTypeView::NonNullTypeParameter { bound: next, .. } = bound.view()
        {
            bound = next;
        }
        riverpod::is_type(
            self.ctx,
            state,
            bound,
            &["AsyncValue", "AsyncData", "AsyncError", "AsyncLoading"],
        )
        .then_some((receiver, flag))
    }

    fn collection_chain(&mut self, node: Expr<'a>) -> Option<usize> {
        let mut wrapper = node.syntax().parent()?;
        let is_case = wrapper.kind() == NodeKind::CaseCondition;
        if is_case {
            wrapper = wrapper.parent()?;
        }
        if wrapper.kind() != NodeKind::ExprCondition {
            return None;
        }
        let ancestor = wrapper.parent()?;
        let (condition, next) = if let Some(branch) = IfElement::cast(ancestor) {
            (
                branch.condition(),
                branch.else_elem().and_then(|element| match element {
                    CollectionElement::If(next) => Some(next.syntax().id()),
                    _ => None,
                }),
            )
        } else {
            let branch = MapIfElement::cast(ancestor)?;
            (
                branch.condition(),
                branch.else_entry().and_then(|element| match element {
                    MapElement::If(next) => Some(next.syntax().id()),
                    _ => None,
                }),
            )
        };
        let index = self.chain(ancestor.id(), condition.span());
        if let Some(next) = next {
            self.continuations.insert(next, index);
        }
        if is_case {
            self.chains[index].valid = false;
            return None;
        }
        Some(index)
    }
}

fn same_receiver<'a>(
    mut left: Expr<'a>,
    mut right: Expr<'a>,
    state: &SemanticState<'a>,
) -> bool {
    loop {
        if same_resolved_element(left, false, right, false, state) {
            return true;
        }
        // Generic member reads share a declaration, not the SDK's occurrence identity.
        let (Some(a), Some(b)) = (
            state.referenced_declaration(left, false),
            state.referenced_declaration(right, false),
        ) else {
            return false;
        };
        if a.owner != b.owner || a.selector != b.selector {
            return false;
        }
        match (left.unparenthesized(), right.unparenthesized()) {
            (Expr::Field(a), Expr::Field(b)) => {
                left = a.object();
                right = b.object();
            }
            (Expr::Ident(_), Expr::Ident(_)) => return true,
            _ => return false,
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.chains
            .iter()
            .filter(|chain| chain.valid && chain.states.count_ones() >= 2)
            .map(|chain| {
                self.ctx.diagnostic(
                    "Use `AsyncValue.when` instead of branching on loading, error and data states manually.",
                    chain.span,
                )
            })
            .collect()
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        let Stmt::If(branch) = node else {
            return;
        };
        let index = self.chain(node.syntax().id(), branch.condition().span());
        if let IfCondition::Expr(condition) = branch.condition() {
            self.conditions.insert(condition.syntax().id(), index);
        } else {
            self.chains[index].valid = false;
        }
        if let Some(next @ Stmt::If(_)) = branch.else_branch() {
            self.continuations.insert(next.syntax().id(), index);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Some(index) = self.conditions.remove(&node.syntax().id())
            .or_else(|| self.collection_chain(node))
        {
            let check = self.state_check(node, state);
            let chain = &mut self.chains[index];
            if let Some((receiver, flag)) = check {
                if chain.receiver.is_some_and(|first| {
                    !same_receiver(first, receiver, state)
                }) {
                    chain.valid = false;
                }
                chain.receiver.get_or_insert(receiver);
                chain.states |= flag;
            } else {
                chain.valid = false;
            }
        }
        if let Expr::Conditional(conditional) = node {
            let index = self.chain(node.syntax().id(), conditional.condition().span());
            self.conditions.insert(conditional.condition().syntax().id(), index);
            if let next @ Expr::Conditional(_) = conditional.else_expr().unparenthesized() {
                self.continuations.insert(next.syntax().id(), index);
            }
        }
    }
}
