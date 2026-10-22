//! Reuse a local for stable SDK arithmetic and String.length/int parity getters.
//! Calls, user getters, allocations and overloaded operators are excluded.
//! Whole-scope mutation facts deliberately reject even later/closure writes.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct UseExistingVariable;

impl Rule for UseExistingVariable {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            references: ctx.local_declarations(program),
            mutated: ctx.mutated_declarations(program),
            stable: HashSet::new(),
            expressions: FxHashMap::default(),
            const_ancestors: FxHashMap::default(),
            initializers: FxHashMap::default(),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Candidate<'a> {
    declaration: LocalVarDecl<'a>,
    value: Expr<'a>,
    nodes: usize,
}

#[derive(Clone, Copy)]
struct ExpressionFacts {
    safe: bool,
    numeric: bool,
    nodes: usize,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: &'a HashMap<usize, NodeId>,
    mutated: &'a HashSet<NodeId>,
    stable: HashSet<NodeId>,
    expressions: FxHashMap<NodeId, ExpressionFacts>,
    const_ancestors: FxHashMap<NodeId, bool>,
    initializers: FxHashMap<NodeId, (LocalVarDecl<'a>, NodeId)>,
    candidates: Vec<Candidate<'a>>,
    diagnostics: Vec<Diagnostic>,
}

use crate::lint::support::block_scope::same_block;

impl<'a> Collector<'a> {
    fn safe_number(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
        if let Some(facts) = self.expressions.get(&expr.syntax().id()) {
            return facts.safe;
        }
        let mut pending = vec![(expr, false)];
        while let Some((node, exiting)) = pending.pop() {
            let id = node.syntax().id();
            if self.expressions.contains_key(&id) {
                continue;
            }
            let children = match node {
                Expr::Paren(paren) => (Some(paren.expr()), None),
                Expr::Binary(binary)
                    if matches!(binary.op(), BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) =>
                {
                    (Some(binary.left()), Some(binary.right()))
                }
                Expr::Field(field)
                    if !field.is_null_safe()
                        && matches!(field.field().name(), "length" | "isOdd" | "isEven") =>
                {
                    (Some(field.object()), None)
                }
                _ => (None, None),
            };
            if !exiting {
                pending.push((node, true));
                for child in [children.1, children.0].into_iter().flatten() {
                    pending.push((child, false));
                }
                continue;
            }
            let facts = |child: Expr<'a>| self.expressions[&child.syntax().id()];
            let numeric = |ty: falcon_analyze::ResolvedType| {
                matches!(ty.view(), ResolvedTypeView::Interface { identity, nullable: false, .. }
                    if identity.is_sdk("dart:core", "int") || identity.is_sdk("dart:core", "double"))
            };
            let safe = children
                .0
                .into_iter()
                .chain(children.1)
                .all(|child| facts(child).safe)
                && match node {
                    Expr::Paren(_) | Expr::IntLit(_) | Expr::DoubleLit(_) => true,
                    Expr::Ident(name) => {
                        self.references.get(&name.span().start).is_some_and(|id| {
                            matches!(state.infer(node).view(), ResolvedTypeView::Interface { identity, nullable: false, .. }
                                if ((identity.is_sdk("dart:core", "int") || identity.is_sdk("dart:core", "double"))
                                    && self.stable.contains(id))
                                    || (identity.is_sdk("dart:core", "String") && !self.mutated.contains(id)))
                        })
                    }
                    Expr::Binary(binary) => {
                        // Safe int/double operands retain a numeric result for these SDK operators.
                        matches!(binary.op(), BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
                            && facts(binary.left()).numeric
                            && facts(binary.right()).numeric
                    }
                    Expr::Field(field) if !field.is_null_safe() => {
                        // Prove SDK getter results even when the lightweight index omits signatures.
                        match field.field().name() {
                            "length" => matches!(state.infer(field.object()).view(),
                                ResolvedTypeView::Interface { identity, nullable: false, .. }
                                    if identity.is_sdk("dart:core", "String"))
                                && matches!(field.object().unparenthesized(), Expr::Ident(_)),
                            "isOdd" | "isEven" => matches!(state.infer(field.object()).view(),
                                ResolvedTypeView::Interface { identity, nullable: false, .. }
                                    if identity.is_sdk("dart:core", "int"))
                                || matches!(field.object().unparenthesized(), Expr::Field(inner)
                                    if inner.field().name() == "length"),
                            _ => false,
                        }
                    }
                    _ => false,
                };
            let is_numeric = safe
                && match node {
                    Expr::Paren(paren) => facts(paren.expr()).numeric,
                    Expr::Binary(_) | Expr::IntLit(_) | Expr::DoubleLit(_) => true,
                    _ => numeric(state.infer(node)),
                };
            let nodes = 1 + children
                .0
                .into_iter()
                .chain(children.1)
                .map(|child| facts(child).nodes)
                .sum::<usize>();
            self.expressions.insert(
                id,
                ExpressionFacts {
                    safe,
                    numeric: is_numeric,
                    nodes,
                },
            );
        }
        self.expressions[&expr.syntax().id()].safe
    }

    fn in_const_context(&mut self, node: Expr<'a>) -> bool {
        let mut ancestor = node.syntax().parent();
        if let Some(cached) = ancestor.and_then(|parent| self.const_ancestors.get(&parent.id())) {
            return *cached;
        }
        let mut pending = Vec::new();
        let mut is_const = false;
        while let Some(parent) = ancestor {
            if let Some(cached) = self.const_ancestors.get(&parent.id()) {
                is_const = *cached;
                break;
            }
            pending.push(parent.id());
            if parent
                .first_token()
                .is_some_and(|token| token.kind() == falcon_syntax::token::TokenKind::Const)
            {
                is_const = true;
                break;
            }
            if Stmt::cast(parent).is_some() {
                break;
            }
            ancestor = parent.parent();
        }
        for id in pending {
            self.const_ancestors.insert(id, is_const);
        }
        is_const
    }

    fn equivalent(&self, left: Expr<'a>, right: Expr<'a>) -> bool {
        let left = left.unparenthesized();
        let right = right.unparenthesized();
        if !crate::lint::expressions::equivalent(left, right, self.ctx) {
            return false;
        }
        // Shared token equality handles syntax; additionally require each local
        // reference to identify the same declaration, without allocating a map.
        let tokens = |expr: Expr<'a>| {
            self.ctx
                .tree()
                .tokens_in(self.ctx.source(), expr.span())
                .filter(|token| !token.is_trivia() || token.kind().is_comment())
        };
        tokens(left).zip(tokens(right)).all(|(a, b)| {
            self.references.get(&a.span().start) == self.references.get(&b.span().start)
        })
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        if declaration.is_late() {
            return;
        }
        for variable in declaration.declarators() {
            if self.mutated.contains(&variable.name().syntax().id()) {
                continue;
            }
            if let Some(value) = variable.initializer() {
                self.initializers.insert(
                    value.syntax().id(),
                    (declaration, variable.name().syntax().id()),
                );
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Some((declaration, variable)) = self.initializers.remove(&node.syntax().id()) {
            // Earlier declarators now have their evaluated types in the semantic environment.
            if declaration.var_type().is_none()
                && matches!(node.unparenthesized(), Expr::Binary(_) | Expr::Field(_))
                && self.safe_number(node, state)
            {
                self.candidates.push(Candidate {
                    declaration,
                    value: node,
                    nodes: self.expressions[&node.unparenthesized().syntax().id()].nodes,
                });
            }
            self.stable.insert(variable);
            self.expressions.clear();
        }
        if !matches!(node, Expr::Binary(_) | Expr::Field(_))
            || self.in_const_context(node)
            || !self.safe_number(node, state)
        {
            return;
        }
        for candidate in &self.candidates {
            // Reject differently shaped prefixes before comparing tokens; ignore whitespace and outer parentheses.
            if candidate.nodes == self.expressions[&node.unparenthesized().syntax().id()].nodes
                && same_block(candidate.declaration.syntax(), node.syntax())
                && self.equivalent(candidate.value, node)
            {
                self.diagnostics.push(self.ctx.diagnostic("Reuse the existing local instead of repeating this computation.", node.span()));
                break;
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
