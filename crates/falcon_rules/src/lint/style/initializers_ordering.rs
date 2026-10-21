//! Requires field initializers to follow their containing type's field declaration
//! order by default. Explicit `order: ascending` or `descending` selects
//! alphabetical ordering. Only literal values and constructor-parameter reads
//! can be reordered because initializer expressions evaluate in written order.
//! Other expressions, assertions, and `super`/`this` constructor calls keep their
//! positions and break ordering runs; unknown fields also break declaration-order
//! runs rather than guessing their order.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

const RULE: &str = "initializers-ordering";

#[derive(Default)]
pub struct InitializersOrdering;

impl Rule for InitializersOrdering {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let order = Order::configured(ctx, RULE);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            order,
            fields: HashMap::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    order: Option<Order>,
    fields: HashMap<&'a str, usize>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.fields.clear();
        if self.order.is_none()
            && let Some(members) = crate::lint::declarations::members_of(node)
        {
            for member in members {
                if let ClassMember::Field(field) = member
                    && !field.is_static()
                {
                    for declarator in field.declarators() {
                        let index = self.fields.len();
                        self.fields.insert(declarator.name().name(), index);
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        let mut previous = None;
        for initializer in node.initializers {
            let ConstructorInitializer::FieldInit(field) = initializer else {
                previous = None;
                continue;
            };
            let safe = match field.value() {
                Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) | Expr::SymbolLit(_) => true,
                Expr::StringLit(string) => string.interpolations().next().is_none(),
                Expr::Ident(identifier) => node.params.params().any(|param| param.name().name() == identifier.name()),
                _ => false,
            };
            if !safe {
                previous = None;
                continue;
            }
            let name = field.field();
            let index = if self.order.is_some() {
                0
            } else if let Some(index) = self.fields.get(name.name()) {
                *index
            } else {
                previous = None;
                continue;
            };
            if previous.is_some_and(|(previous_name, previous_index)| match self.order {
                Some(order) => order.is_out_of_order(previous_name, name.name()),
                None => previous_index > index,
            }) {
                let message = if self.order.is_some() {
                    "Reorder field initializers to follow the configured alphabetical order."
                } else {
                    "Reorder field initializers to follow field declaration order."
                };
                self.diagnostics.push(self.ctx.diagnostic(message, name.span()));
            }
            previous = Some((name.name(), index));
        }
        StructuralTraversal::Continue
    }
}
}
