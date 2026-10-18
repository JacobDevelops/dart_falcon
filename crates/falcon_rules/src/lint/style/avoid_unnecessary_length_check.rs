//! Empty collections already skip `for-in` and `forEach` bodies.
//!
//! Only a sole iteration guarded by a positive emptiness test is redundant.
//! Restrict receivers to local, SDK collection-typed identifiers: getters,
//! lazy iterables, unknown types and custom collection APIs are not assumed pure.
//! A forEach callback must be a closure, not an expression evaluated for effects.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryLengthCheck;

impl Rule for AvoidUnnecessaryLengthCheck {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

fn nonempty_receiver(condition: Expr<'_>) -> Option<Expr<'_>> {
    match condition.unparenthesized() {
        Expr::Field(field) if field.field().name() == "isNotEmpty" && !field.is_null_safe() => {
            Some(field.object())
        }
        Expr::Binary(binary) => {
            let (length, literal, reversed) = match binary.left().unparenthesized() {
                Expr::IntLit(_) => (binary.right(), binary.left(), true),
                _ => (binary.left(), binary.right(), false),
            };
            let Expr::Field(field) = length.unparenthesized() else {
                return None;
            };
            let Expr::IntLit(value) = literal.unparenthesized() else {
                return None;
            };
            let value = value.value().parse::<u64>().ok()?;
            let nonempty = matches!(
                (binary.op(), value, reversed),
                (BinaryOp::NotEq, 0, _)
                    | (BinaryOp::Gt, 0, false)
                    | (BinaryOp::GtEq, 1, false)
                    | (BinaryOp::Lt, 0, true)
                    | (BinaryOp::LtEq, 1, true)
            );
            (nonempty && field.field().name() == "length" && !field.is_null_safe())
                .then(|| field.object())
        }
        _ => None,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::If(statement) = node else { return };
        if statement.else_branch().is_some() {
            return;
        }
        let IfCondition::Expr(condition) = statement.condition() else {
            return;
        };
        let Some(receiver) = nonempty_receiver(condition) else {
            return;
        };
        let Expr::Ident(identifier) = receiver.unparenthesized() else {
            return;
        };
        if !state.environment.is_bound(identifier.name()) || state.is_own_field(identifier.name()) {
            return;
        }
        let ty = state.infer(receiver);
        if ty.nullable() {
            return;
        }
        let iterable = ty.interface("dart:core", "List") || ty.interface("dart:core", "Set");
        if !iterable && !ty.interface("dart:core", "Map") {
            return;
        }
        let body = match statement.then_branch() {
            Stmt::Block(block) if block.stmts().len() == 1 => block.stmts().next().unwrap(),
            other => other,
        };
        let iterated = match body {
            Stmt::For(loop_statement) if iterable && !loop_statement.is_await() => {
                match loop_statement.header().init() {
                    Some(ForInit::ForIn(init)) => Some(init.iterable()),
                    Some(ForInit::PatternForIn(init)) => Some(init.iterable()),
                    _ => None,
                }
            }
            Stmt::Expr(expression) => {
                let Expr::Call(call) = expression.expr().unparenthesized() else {
                    return;
                };
                let Expr::Field(field) = call.callee().unparenthesized() else {
                    return;
                };
                if field.field().name() != "forEach"
                    || field.is_null_safe()
                    || !call.args().named().is_empty()
                    || call.args().positional().len() != 1
                    || !matches!(
                        call.args().positional().next().unwrap().unparenthesized(),
                        Expr::FuncExpr(_)
                    )
                {
                    return;
                }
                Some(field.object())
            }
            _ => None,
        };
        if let Some(iterated) = iterated
            && let Expr::Ident(other) = iterated.unparenthesized()
            && other.name() == identifier.name()
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-unnecessary-length-check",
                "Remove this emptiness guard: iteration already skips empty collections.",
                condition.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
