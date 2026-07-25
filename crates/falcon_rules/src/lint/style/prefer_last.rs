//! Flags manual last-element indexing in favor of the `.last` getter.
//!
//! Catches the `xs[xs.length - 1]` idiom, where the receiver and the collection
//! whose `length` is read are the same identifier. Iterables expose a dedicated
//! `.last` getter that reads more clearly, avoids repeating the receiver, and
//! removes the off-by-one arithmetic that manual indexing invites. The match is
//! deliberately narrow: it requires a literal `- 1` subtracted from `<name>.length`
//! indexing `<name>`, so unrelated index expressions are left alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferLast;

impl Rule for PreferLast {
    fn name(&self) -> &'static str {
        "prefer-last"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Index {
            object,
            index,
            span,
            ..
        } = node
            && is_length_minus_one(object, index)
        {
            self.diags.push(Diagnostic::new(
                "prefer-last",
                Severity::Warning,
                "Prefer .last over [length - 1] to access the last element",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}

/// Matches `ident[ident.length - 1]` where both idents are the same name.
fn is_length_minus_one(object: &Expr, index: &Expr) -> bool {
    let obj_name = match object {
        Expr::Ident(id) => &id.name,
        _ => return false,
    };
    match index {
        Expr::Binary {
            op: BinaryOp::Sub,
            left,
            right,
            ..
        } => {
            if !matches!(right.as_ref(), Expr::IntLit { value, .. } if value == "1") {
                return false;
            }
            match left.as_ref() {
                Expr::Field {
                    object: len_obj,
                    field,
                    ..
                } => {
                    field.name == "length"
                        && matches!(len_obj.as_ref(), Expr::Ident(id) if &id.name == obj_name)
                }
                _ => false,
            }
        }
        _ => false,
    }
}
