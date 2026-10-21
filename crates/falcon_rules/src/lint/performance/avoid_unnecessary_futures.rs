//! Prefer synchronous declarations when an async Future only returns scalars.
//!
//! Only explicitly typed SDK Future top-level/local functions with known scalar
//! returns are reported. Methods/getters are excluded to preserve inherited
//! contracts. Awaiting, throwing, calls, accesses, operators and patterns exclude
//! the body.
//! No automatic fix is offered: changing a public return type requires callers
//! to be updated as well.

use std::collections::{HashMap, HashSet};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::function_bodies::{BodyOwner, FunctionBodies};

#[derive(Default)]
pub struct AvoidUnnecessaryFutures;

impl Rule for AvoidUnnecessaryFutures {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            bodies: FunctionBodies::new(program),
            candidates: HashMap::new(),
            unsafe_bodies: HashSet::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies<'a>,
    candidates: HashMap<usize, Span>,
    unsafe_bodies: HashSet<usize>,
}

impl<'a> Collector<'a> {
    fn returned(&mut self, value: Expr<'a>, state: &SemanticState<'a>) {
        let Some(body) = self.bodies.enclosing(&value.span()) else {
            return;
        };
        let Some(return_type) = body.return_type else {
            return;
        };
        if body.async_marker != AsyncMarker::Async
            || !matches!(
                body.owner,
                BodyOwner::TopLevelFunction | BodyOwner::LocalFunction
            )
            || is_main(value.syntax())
        {
            return;
        }
        let declared = state
            .model
            .resolve_type_in(return_type, &state.type_parameters);
        if !declared.interface("dart:async", "Future") || declared.nullable() {
            return;
        }
        let ty = state.infer(value);
        if !["int", "double", "num", "bool", "String"]
            .iter()
            .any(|name| ty.interface("dart:core", name))
            || ty.nullable()
        {
            self.unsafe_bodies.insert(body.span.start);
            return;
        }
        self.candidates.insert(body.span.start, return_type.span());
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    self.returned(value, state);
                } else if let Some(body) = self.bodies.enclosing(&node.span()) {
                    self.unsafe_bodies.insert(body.span.start);
                }
            }
            Stmt::LocalVar(declaration) if declaration.is_late() => {
                if let Some(body) = self.bodies.enclosing(&node.span()) {
                    self.unsafe_bodies.insert(body.span.start);
                }
            }
            Stmt::Throw(_)
            | Stmt::Rethrow(_)
            | Stmt::Assert(_)
            | Stmt::Yield(_)
            | Stmt::For(_)
            | Stmt::While(_)
            | Stmt::DoWhile(_)
            | Stmt::Switch(_) => {
                if let Some(body) = self.bodies.enclosing(&node.span()) {
                    self.unsafe_bodies.insert(body.span.start);
                }
            }
            _ => {}
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        _matched: Option<&falcon_analyze::ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
        if let Some(body) = self.bodies.enclosing(&node.span()) {
            self.unsafe_bodies.insert(body.span.start);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some(body) = self.bodies.enclosing(&node.span()) else {
            return;
        };
        let start = body.span.start;
        let arrow = body.arrow_expression == Some(node.span());
        let trivial = match node {
            Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Paren(_)
            | Expr::FuncExpr(_) => true,
            Expr::StringLit(value) => value.interpolations().next().is_none(),
            Expr::Ident(identifier) => {
                body.owner == BodyOwner::TopLevelFunction
                    && state.environment.is_bound(identifier.name())
            }
            _ => false,
        };
        if !trivial {
            self.unsafe_bodies.insert(start);
        }
        if arrow {
            self.returned(node, state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut candidates: Vec<_> = self
            .candidates
            .into_iter()
            .filter(|(body, _)| !self.unsafe_bodies.contains(body))
            .collect();
        candidates.sort_unstable_by_key(|(_, span)| span.start);
        candidates.into_iter().map(|(_, span)| self.ctx.diagnostic("This async declaration only returns immediate values. Make it synchronous and return the value type instead of Future, updating its callers.", span)).collect()
    }
}

fn is_main(mut node: SyntaxNode<'_>) -> bool {
    loop {
        if let Some(function) = FunctionDecl::cast(node) {
            return function.name().name() == "main";
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}
