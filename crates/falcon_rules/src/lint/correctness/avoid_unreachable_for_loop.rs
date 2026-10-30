//! Reports iteration of a collection in a branch that requires it to be empty.
//!
//! Only resolved local SDK collections and immediate iteration are inspected.
//! Fields/getters, intervening statements and unknown collection types may change
//! between the guard and iteration, so they are deliberately left alone.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnreachableForLoop;

impl Rule for AvoidUnreachableForLoop {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            declarations: ctx.local_declarations(program),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: &'a HashMap<usize, NodeId>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::If(node) = statement else { return };
        let IfCondition::Expr(condition) = node.condition() else {
            return;
        };
        let Some(checked) = empty_collection(condition) else {
            return;
        };
        let ty = state.infer(Expr::Ident(checked));
        if !["List", "Set", "Map"]
            .iter()
            .any(|name| ty.interface("dart:core", name))
        {
            return;
        }
        let body = match node.then_branch() {
            Stmt::Block(block) => {
                let mut statements = block.stmts();
                match (statements.next(), statements.next()) {
                    (Some(statement), None) => statement,
                    _ => return,
                }
            }
            statement => statement,
        };
        let iterable = match body {
            Stmt::For(loop_node) => match loop_node.header().init() {
                Some(ForInit::ForIn(header)) => Some(header.iterable()),
                Some(ForInit::PatternForIn(header)) => Some(header.iterable()),
                _ => None,
            },
            Stmt::Expr(statement) => match statement.expr().unparenthesized() {
                Expr::Call(call) => match call.callee().unparenthesized() {
                    Expr::Field(field) if field.field().name() == "forEach" => Some(field.object()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(Expr::Ident(iterated)) = iterable.map(Expr::unparenthesized) else {
            return;
        };
        let Some(checked_declaration) = self.declarations.get(&checked.span().start) else {
            return;
        };
        if self.declarations.get(&iterated.span().start) == Some(checked_declaration) {
            self.diags.push(self.ctx.diagnostic("This condition requires the collection to be empty, so the iteration body cannot run.", condition.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}

fn empty_collection(condition: Expr<'_>) -> Option<Identifier<'_>> {
    let field = match condition.unparenthesized() {
        Expr::Field(field) if field.field().name() == "isEmpty" => field,
        Expr::Unary(unary) if unary.op() == UnaryOp::Bang => {
            let Expr::Field(field) = unary.operand().unparenthesized() else {
                return None;
            };
            if field.field().name() != "isNotEmpty" {
                return None;
            }
            field
        }
        Expr::Binary(binary) if binary.op() == BinaryOp::EqEq => {
            let ((Expr::Field(property), Expr::IntLit(zero))
            | (Expr::IntLit(zero), Expr::Field(property))) = (
                binary.left().unparenthesized(),
                binary.right().unparenthesized(),
            )
            else {
                return None;
            };
            if property.field().name() != "length" || zero.value() != "0" {
                return None;
            }
            property
        }
        _ => return None,
    };
    match field.object().unparenthesized() {
        Expr::Ident(identifier) => Some(identifier),
        _ => None,
    }
}
