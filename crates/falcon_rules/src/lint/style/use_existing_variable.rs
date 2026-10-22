//! Reuse a local for stable SDK arithmetic and String.length/int parity getters.
//! Calls, user getters, allocations and overloaded operators are excluded.
//! Whole-scope mutation facts deliberately reject even later/closure writes.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

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
            references: local_declarations(program),
            mutated: ctx.mutated_declarations(program),
            stable: HashSet::new(),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Candidate<'a> {
    declaration: LocalVarDecl<'a>,
    value: Expr<'a>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    mutated: &'a HashSet<NodeId>,
    stable: HashSet<NodeId>,
    candidates: Vec<Candidate<'a>>,
    diagnostics: Vec<Diagnostic>,
}

use crate::lint::support::block_scope::same_block;

impl<'a> Collector<'a> {
    fn safe_number(&self, expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let mut safe = true;
        visit::for_each_expr(expr, &mut |node| {
            if !safe {
                return;
            }
            safe = match node {
                Expr::Paren(_) | Expr::IntLit(_) | Expr::DoubleLit(_) => true,
                Expr::Ident(name) => {
                    self.references.get(&name.span().start).is_some_and(|id| {
                        matches!(state.infer(node), ResolvedType::Interface { identity, nullable: false, .. }
                            if ((identity.is_sdk("dart:core", "int") || identity.is_sdk("dart:core", "double"))
                                && self.stable.contains(id))
                                || (identity.is_sdk("dart:core", "String") && !self.mutated.contains(id)))
                    })
                }
                Expr::Binary(binary) => {
                    matches!(binary.op(), BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
                        && matches!(state.infer(node), ResolvedType::Interface { identity, nullable: false, .. }
                            if identity.is_sdk("dart:core", "int") || identity.is_sdk("dart:core", "double"))
                }
                Expr::Field(field) if !field.is_null_safe() => {
                    // Prove built-in results even when the lightweight SDK index
                    // does not carry their getter result signatures.
                    match field.field().name() {
                        "length" => matches!(state.infer(field.object()),
                            ResolvedType::Interface { identity, nullable: false, .. }
                                if identity.is_sdk("dart:core", "String"))
                            && matches!(field.object().unparenthesized(), Expr::Ident(_)),
                        "isOdd" | "isEven" => matches!(state.infer(field.object()),
                            ResolvedType::Interface { identity, nullable: false, .. }
                                if identity.is_sdk("dart:core", "int"))
                            || matches!(field.object().unparenthesized(), Expr::Field(inner)
                                if inner.field().name() == "length"),
                        _ => false,
                    }
                }
                _ => false,
            };
        });
        safe
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
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
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
                if declaration.var_type().is_none()
                    && matches!(value.unparenthesized(), Expr::Binary(_) | Expr::Field(_))
                    && self.safe_number(value, state)
                {
                    self.candidates.push(Candidate { declaration, value });
                }
                self.stable.insert(variable.name().syntax().id());
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let mut ancestor = node.syntax().parent();
        while let Some(parent) = ancestor {
            if parent
                .first_token()
                .is_some_and(|token| token.kind() == falcon_syntax::token::TokenKind::Const)
            {
                return;
            }
            if Stmt::cast(parent).is_some() {
                break;
            }
            ancestor = parent.parent();
        }
        if !matches!(node, Expr::Binary(_) | Expr::Field(_)) || !self.safe_number(node, state) {
            return;
        }
        for candidate in &self.candidates {
            if same_block(candidate.declaration.syntax(), node.syntax())
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
