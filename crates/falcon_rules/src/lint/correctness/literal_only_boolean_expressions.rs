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
use falcon_syntax::cst::*;

use crate::lint::const_eval::{constant_in_context, deferred_prefixes};
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

const MESSAGE: &str = "The Boolean expression has a constant value.";

#[derive(Default)]
pub struct LiteralOnlyBooleanExpressions;

impl Rule for LiteralOnlyBooleanExpressions {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
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

impl<'a> Collector<'_, 'a> {
    fn report_if_literal(&mut self, condition: Expr<'a>, span: Span, state: &SemanticState<'a>) {
        if self.only_literals(condition, state) {
            self.diags.push(self.ctx.diagnostic(MESSAGE, span));
        }
    }

    fn check_guard(&mut self, guard: Option<Expr<'a>>, state: &SemanticState<'a>) {
        if let Some(guard) = guard {
            let start = self
                .ctx
                .tree()
                .top_level_token_at(self.ctx.source(), guard.span().start)
                .and_then(SyntaxToken::prev_significant)
                .filter(|token| token.kind() == falcon_syntax::TokenKind::When)
                .map_or(guard.span().start, |token| token.offset());
            self.report_if_literal(guard, Span::new(start, guard.span().end), state);
        }
    }

    fn check_if_condition(&mut self, condition: IfCondition<'a>, state: &SemanticState<'a>) {
        if let IfCondition::Case(node) = condition {
            self.check_guard(node.guard(), state);
        }
    }

    fn check_element(&mut self, element: CollectionElement<'a>, state: &SemanticState<'a>) {
        match element {
            CollectionElement::If(node) => {
                self.check_if_condition(node.condition(), state);
                self.check_element(node.then_elem(), state);
                if let Some(else_elem) = node.else_elem() {
                    self.check_element(else_elem, state);
                }
            }
            CollectionElement::For(node) => self.check_element(node.element(), state),
            CollectionElement::Expr(_)
            | CollectionElement::NullAware(_)
            | CollectionElement::Spread(_) => {}
        }
    }

    fn check_map_element(&mut self, element: MapElement<'a>, state: &SemanticState<'a>) {
        match element {
            MapElement::If(node) => {
                self.check_if_condition(node.condition(), state);
                self.check_map_element(node.then_entry(), state);
                if let Some(else_entry) = node.else_entry() {
                    self.check_map_element(else_entry, state);
                }
            }
            MapElement::For(node) => self.check_map_element(node.entry(), state),
            MapElement::Entry(_) | MapElement::Spread(_) => {}
        }
    }

    /// Whether `expression` is composed only of literals, mirroring the
    /// official lint: prefix and binary operators recurse (only the left side
    /// of `??`), and `is` checks recurse unless the tested type is a type
    /// parameter. Strings must have a computable constant value.
    fn only_literals(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match expression.unparenthesized() {
                Expr::IntLit(_)
                | Expr::DoubleLit(_)
                | Expr::BoolLit(_)
                | Expr::NullLit(_)
                | Expr::SymbolLit(_)
                | Expr::List(_)
                | Expr::Map(_)
                | Expr::Set(_)
                | Expr::Record(_) => {}
                Expr::StringLit(_) if constant_in_context(expression, &self.deferred, state) => {}
                Expr::Unary(node) => pending.push(node.operand()),
                Expr::Binary(node) if node.op() == BinaryOp::NullCoalesce => {
                    pending.push(node.left())
                }
                Expr::Binary(node) => {
                    pending.push(node.left());
                    pending.push(node.right());
                }
                Expr::Is(node) => {
                    if let DartType::Named(named) = node.dart_type() {
                        let mut segments = named.segments();
                        if let (Some(segment), None) = (segments.next(), segments.next())
                            && state.type_parameters.contains(segment.name())
                        {
                            return false;
                        }
                    }
                    pending.push(node.expr());
                }
                _ => return false,
            }
        }
        true
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::If(statement) => match statement.condition() {
                IfCondition::Expr(condition) => {
                    self.report_if_literal(condition, statement.span(), state);
                }
                IfCondition::Case(_) => self.check_if_condition(statement.condition(), state),
            },
            Stmt::While(statement) => {
                if !matches!(
                    statement.condition().unparenthesized(),
                    Expr::BoolLit(node) if node.value()
                ) {
                    self.report_if_literal(statement.condition(), statement.span(), state);
                }
            }
            Stmt::DoWhile(statement) => {
                self.report_if_literal(statement.condition(), statement.span(), state);
            }
            Stmt::For(statement) => {
                let for_parts = !matches!(
                    statement.header().init(),
                    Some(ForInit::ForIn(_) | ForInit::PatternForIn(_))
                );
                if for_parts && let Some(condition) = statement.header().condition() {
                    self.report_if_literal(condition, statement.span(), state);
                }
            }
            Stmt::Switch(statement) => {
                for case in statement.cases() {
                    for kind in case.cases() {
                        if let SwitchCaseKind::Pattern(label) = kind {
                            self.check_guard(label.guard(), state);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Switch(node) => {
                for arm in node.arms() {
                    self.check_guard(arm.guard(), state);
                }
            }
            Expr::List(node) => {
                for element in node.elements() {
                    self.check_element(element, state);
                }
            }
            Expr::Set(node) => {
                for element in node.elements() {
                    self.check_element(element, state);
                }
            }
            Expr::Map(node) => {
                for element in node.elements() {
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
        let diagnostics = inference::analyze(LiteralOnlyBooleanExpressions::NAME, &LiteralOnlyBooleanExpressions, Path::new("guard_spans.dart"),
        source,
        &FalconConfig::default(),);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["when /* guard */ true", "when 2 > 1"]);
    }

    #[test]
    fn guard_ranges_inside_interpolation_exclude_when() {
        let source = "String f(int x) => '${switch (x) { 1 when /* guard */ true => 1, _ => 0 }}';";
        let diagnostics = inference::analyze(LiteralOnlyBooleanExpressions::NAME, &LiteralOnlyBooleanExpressions, Path::new("interpolated_guard_spans.dart"),
        source,
        &FalconConfig::default(),);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["true"]);
    }
}
