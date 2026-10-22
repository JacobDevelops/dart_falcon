//! Avoids shrink-wrapped Flutter lists nested in rows, columns, or other lists.
//!
//! Shrink wrapping repeatedly measures the list's contents; prefer slivers instead.
//! Only resolved Flutter `ListView` constructors with a literal `true` shrinkWrap
//! argument inside a Flutter Row, Column, or ListView child argument are flagged.
//! Unknown flags, false values, unrelated classes, and standalone lists are ignored.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidShrinkWrapInLists;

impl Rule for AvoidShrinkWrapInLists {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            children: Vec::new(),
            functions: Vec::new(),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    children: Vec<Span>,
    functions: Vec<Span>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        self.children.retain(|span| span.end >= node.span().start);
        self.functions.retain(|span| span.end >= node.span().start);
        if let Expr::FuncExpr(function) = node
            && self
                .children
                .iter()
                .any(|parent| parent.contains(function.span()))
        {
            self.functions.push(function.span());
        }
        if !matches!(node, Expr::Call(_) | Expr::New(_)) {
            return;
        }
        let Some((identity, _, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        let is_list = index.is_package_declaration(&identity, "flutter", "ListView");
        if is_list
            && self.children.iter().any(|parent| {
                parent.contains(node.span())
                    && !self.functions.iter().any(|function| {
                        parent.contains(*function) && function.contains(node.span())
                    })
            })
            && let Some(argument) = args.named().find(|arg| arg.name().name() == "shrinkWrap")
            && matches!(argument.value().unparenthesized(), Expr::BoolLit(value) if value.value())
        {
            self.diags.push(self.ctx.diagnostic("Avoid shrink wrapping a nested list; use slivers instead.", argument.span()));
        }
        if is_list
            || index.is_package_declaration(&identity, "flutter", "Column")
            || index.is_package_declaration(&identity, "flutter", "Row")
        {
            self.children.extend(
                args.named()
                    .filter(|argument| argument.name().name() == "children")
                    .map(|argument| argument.value().span()),
            );
        }
    }
}

