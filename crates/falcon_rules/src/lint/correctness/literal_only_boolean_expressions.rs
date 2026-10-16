//! Flags conditions built only from literals, whose value never changes.
//!
//! An `if`, `while`, `do`-`while`, or `for` condition (or a pattern `when`
//! guard) made solely of literals combined with operators always evaluates the
//! same way, so the branch is either dead or unconditional. `while (true)` is
//! allowed as the idiomatic infinite loop. Guard diagnostics include the `when` keyword.
//!
//! ```dart
//! if (1 > 2) {}          // flagged
//! while (true && false) {} // flagged
//! while (true) {}        // ok
//! if (x > 2) {}          // ok
//! ```
//!
//! Adopted from the official `literal_only_boolean_expressions` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{constant_in_context, deferred_prefixes};
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

const RULE: &str = "literal-only-boolean-expressions";
const MESSAGE: &str = "The Boolean expression has a constant value.";

#[derive(Default)]
pub struct LiteralOnlyBooleanExpressions;

impl Rule for LiteralOnlyBooleanExpressions {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
            deferred: deferred_prefixes(program),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
    deferred: Vec<&'a str>,
}

impl Collector<'_, '_> {
    fn report_if_literal(&mut self, condition: &Expr, span: Span, state: &SemanticState<'_>) {
        if self.only_literals(condition, state) {
            self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
        }
    }

    fn check_guard(&mut self, guard: Option<&Expr>, state: &SemanticState<'_>) {
        if let Some(guard) = guard {
            let tokens = self.ctx.tokens();
            let before = tokens.partition_point(|token| token.offset() < guard.span().start);
            let start = tokens[..before]
                .iter()
                .rev()
                .find(|token| !token.is_trivia())
                .filter(|token| token.kind == falcon_syntax::TokenKind::When)
                .map_or(guard.span().start, |token| token.offset());
            self.report_if_literal(guard, Span::new(start, guard.span().end), state);
        }
    }

    fn check_if_condition(&mut self, condition: &IfCondition, state: &SemanticState<'_>) {
        if let IfCondition::Case(_, _, guard) = condition {
            self.check_guard(guard.as_deref(), state);
        }
    }

    fn check_element(&mut self, element: &CollectionElement, state: &SemanticState<'_>) {
        match element {
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                self.check_if_condition(condition, state);
                self.check_element(then_elem, state);
                if let Some(else_elem) = else_elem {
                    self.check_element(else_elem, state);
                }
            }
            CollectionElement::For { element, .. } => self.check_element(element, state),
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
        }
    }

    fn check_map_element(&mut self, element: &MapElement, state: &SemanticState<'_>) {
        match element {
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                self.check_if_condition(condition, state);
                self.check_map_element(then_entry, state);
                if let Some(else_entry) = else_entry {
                    self.check_map_element(else_entry, state);
                }
            }
            MapElement::For { entry, .. } => self.check_map_element(entry, state),
            MapElement::Entry(_) | MapElement::Spread { .. } => {}
        }
    }

    /// Whether `expression` is composed only of literals, mirroring the
    /// official lint: prefix and binary operators recurse (only the left side
    /// of `??`), and `is` checks recurse unless the tested type is a type
    /// parameter. Strings must have a computable constant value.
    fn only_literals(&self, expression: &Expr, state: &SemanticState<'_>) -> bool {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match expression.unparenthesized() {
                Expr::IntLit { .. }
                | Expr::DoubleLit { .. }
                | Expr::BoolLit { .. }
                | Expr::NullLit { .. }
                | Expr::SymbolLit { .. }
                | Expr::List { .. }
                | Expr::Map { .. }
                | Expr::Set { .. }
                | Expr::Record { .. } => {}
                Expr::StringLit(_) if constant_in_context(expression, &self.deferred, state) => {}
                Expr::Unary { operand, .. } => pending.push(operand),
                Expr::Binary {
                    op: BinaryOp::NullCoalesce,
                    left,
                    ..
                } => pending.push(left),
                Expr::Binary { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                Expr::Is {
                    expr, dart_type, ..
                } => {
                    if matches!(dart_type, DartType::Named(named)
                        if matches!(named.segments.as_slice(), [segment]
                            if state.type_parameters.contains(&segment.name)))
                    {
                        return false;
                    }
                    pending.push(expr);
                }
                _ => return false,
            }
        }
        true
    }
}

impl SemanticRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        match node {
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.report_if_literal(condition, statement.span, state);
                }
                IfCondition::Case(..) => self.check_if_condition(&statement.condition, state),
            },
            Stmt::While(statement) => {
                if !matches!(
                    statement.condition.unparenthesized(),
                    Expr::BoolLit { value: true, .. }
                ) {
                    self.report_if_literal(&statement.condition, statement.span, state);
                }
            }
            Stmt::DoWhile(statement) => {
                self.report_if_literal(&statement.condition, statement.span, state);
            }
            Stmt::For(statement) => {
                let for_parts = !matches!(
                    statement.header.init,
                    Some(ForInit::ForIn { .. } | ForInit::PatternForIn { .. })
                );
                if for_parts && let Some(condition) = &statement.header.condition {
                    self.report_if_literal(condition, statement.span, state);
                }
            }
            Stmt::Switch(statement) => {
                for case in &statement.cases {
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(_, guard) = kind {
                            self.check_guard(guard.as_deref(), state);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Switch { arms, .. } => {
                for arm in arms {
                    self.check_guard(arm.guard.as_ref(), state);
                }
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.check_element(element, state);
                }
            }
            Expr::Map { elements, .. } => {
                for element in elements {
                    self.check_map_element(element, state);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn guard_ranges_include_when_and_intervening_comments() {
        let source = "int f(int x) { switch (x) { case 1 when /* guard */ true: return 1; default: return 0; } } int g(int x) => switch (x) { 1 when 2 > 1 => 1, _ => 0 };";
        let diagnostics = inference::analyze(
            &LiteralOnlyBooleanExpressions,
            Path::new("guard_spans.dart"),
            source,
            &FalconConfig::default(),
        );
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["when /* guard */ true", "when 2 > 1"]);
    }
}
