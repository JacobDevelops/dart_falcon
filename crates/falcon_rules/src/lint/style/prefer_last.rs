//! Flags manual last-element indexing in favor of the `.last` getter.
//!
//! Catches the `xs[xs.length - 1]` idiom, where the receiver and the collection
//! whose `length` is read are the same identifier. Iterables expose a dedicated
//! `.last` getter that reads more clearly, avoids repeating the receiver, and
//! removes the off-by-one arithmetic that manual indexing invites. The match is
//! deliberately narrow: it requires a literal `- 1` subtracted from `<name>.length`
//! indexing `<name>`, so unrelated index expressions are left alone.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct PreferLast;

impl Rule for PreferLast {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Index {
            object,
            index,
            span,
            ..
        } = node
            && is_length_minus_one(object, index)
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-last",
                "Prefer .last over [length - 1] to access the last element",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}

/// Matches `ident[ident.length - 1]` where both idents are the same name.
fn is_length_minus_one(object: &Expr, index: &Expr) -> bool {
    let obj_name = match object.unparenthesized() {
        Expr::Ident(id) => &id.name,
        _ => return false,
    };
    match index.unparenthesized() {
        Expr::Binary {
            op: BinaryOp::Sub,
            left,
            right,
            ..
        } => {
            if !matches!(right.unparenthesized(), Expr::IntLit { value, .. } if value == "1") {
                return false;
            }
            match left.unparenthesized() {
                Expr::Field {
                    object: len_obj,
                    field,
                    ..
                } => {
                    field.name == "length"
                        && matches!(len_obj.unparenthesized(), Expr::Ident(id) if &id.name == obj_name)
                }
                _ => false,
            }
        }
        _ => false,
    }
}
