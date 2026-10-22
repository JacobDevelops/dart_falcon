//! Flags repeated `when` guards on syntactically identical switch patterns.
//!
//! Repeating a pattern and guard usually leaves the later arm unreachable.
//! This rule compares guard and pattern tokens, preserving literal contents;
//! Constant, literal, logical-or, and parenthesized patterns are left to
//! `no-duplicate-case-values`. Different patterns may legitimately share a guard.
//! Guards must use stable primitive variable reads and built-in comparisons;
//! unknown getters, overloaded operators, and calls are not considered repeatable.
//! The immutable String properties `length`, `isEmpty`, and `isNotEmpty` are
//! supported on stable String variables.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::equivalent;

#[derive(Default)]
pub struct AvoidDuplicateSwitchCaseConditions;

impl Rule for AvoidDuplicateSwitchCaseConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Switch(switch) = expression {
            self.check(
                switch
                    .arms()
                    .filter_map(|arm| arm.guard().map(|guard| (arm.pattern(), guard))),
                state,
            );
        }
    }

    fn visit_stmt(&mut self, statement: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Switch(switch) = statement {
            self.check(
                switch
                    .cases()
                    .flat_map(|case| case.cases())
                    .filter_map(|case| {
                        if let SwitchCaseKind::Pattern(pattern) = case {
                            pattern.guard().map(|guard| (pattern.pattern(), guard))
                        } else {
                            None
                        }
                    }),
                state,
            );
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        cases: impl Iterator<Item = (Pattern<'a>, Expr<'a>)>,
        state: &SemanticState<'a>,
    ) {
        let mut seen: Vec<(Pattern<'a>, Expr<'a>)> = Vec::new();
        for (pattern, guard) in cases {
            if matches!(
                pattern,
                Pattern::Literal(_)
                    | Pattern::Const(_)
                    | Pattern::LogicalOr(_)
                    | Pattern::ParenPattern(_)
            ) || !repeatable_guard(guard, state)
            {
                continue;
            }
            if seen.iter().any(|(previous_pattern, previous_guard)| {
                same_pattern(*previous_pattern, pattern, self.ctx)
                    && equivalent(*previous_guard, guard, self.ctx)
            }) {
                self.diagnostics.push(self.ctx.diagnostic("Remove or change this duplicate switch pattern and condition.", guard.span()));
            } else {
                seen.push((pattern, guard));
            }
        }
    }
}

fn same_pattern(left: Pattern<'_>, right: Pattern<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    let tokens = |span| {
        ctx.tree()
            .tokens_in(ctx.source(), span)
            .filter(|token| !token.is_trivia())
            .map(|token| (token.kind(), token.text()))
    };
    tokens(left.span()).eq(tokens(right.span()))
}

fn repeatable_guard<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let mut proof = GuardProof {
        state,
        repeatable: true,
    };
    falcon_syntax::cst::visit::Visitor::visit_expr(&mut proof, expression);
    proof.repeatable
}

struct GuardProof<'s, 'a> {
    state: &'s SemanticState<'a>,
    repeatable: bool,
}

impl<'a> falcon_syntax::cst::visit::Visitor<'a> for GuardProof<'_, 'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if !self.repeatable {
            return;
        }
        self.repeatable = match expression {
            Expr::Paren(_) => true,
            Expr::Ident(identifier) => {
                self.state
                    .environment
                    .binding_declaration(identifier.name())
                    .is_some()
                    && !self.state.is_own_field(identifier.name())
                    && self.state.is_variable_read(identifier.name())
                    && primitive(expression, self.state)
            }
            Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => true,
            Expr::StringLit(string) => string.interpolations().next().is_none(),
            Expr::Field(field) if !field.is_null_safe() => {
                matches!(field.field().name(), "length" | "isEmpty" | "isNotEmpty")
                    && matches!(field.object().unparenthesized(), Expr::Ident(_))
                    && self
                        .state
                        .infer(field.object())
                        .interface("dart:core", "String")
            }
            Expr::Unary(unary) if unary.op() == UnaryOp::Bang => self
                .state
                .infer(unary.operand())
                .interface("dart:core", "bool"),
            Expr::Binary(binary) => matches!(
                binary.op(),
                BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::EqEq
                    | BinaryOp::NotEq
                    | BinaryOp::Lt
                    | BinaryOp::LtEq
                    | BinaryOp::Gt
                    | BinaryOp::GtEq
            ),
            _ => false,
        };
        if self.repeatable {
            falcon_syntax::cst::visit::walk_expr(self, expression);
        }
    }
}

fn primitive(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let ty = state.infer(expression);
    !ty.nullable()
        && ["bool", "int", "double", "num", "String"]
            .iter()
            .any(|name| ty.interface("dart:core", name))
}
