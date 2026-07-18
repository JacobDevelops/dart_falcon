//! Flags an `addAll` cascade on a collection literal that a spread would express.
//!
//! The spread operator (`[a, b, ...x]`) folds another collection's elements into
//! a literal in one expression, so `[a, b]..addAll(x)` is unnecessarily verbose
//! and reads as a construction followed by mutation rather than a single
//! collection. The rule matches each `..addAll(...)` section cascaded onto a
//! list, set, or map literal that takes exactly one positional argument and no
//! named arguments; other shapes and non-literal receivers are ignored.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferSpreadCollections;

impl Rule for PreferSpreadCollections {
    fn name(&self) -> &'static str {
        "prefer-spread-collections"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "prefer-spread-collections",
            Severity::Warning,
            "Use the spread operator '...' instead of 'addAll'.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Cascade {
            object, sections, ..
        } = node
            && is_collection_literal(object)
        {
            for section in sections {
                // A section reports at most once: multiple `addAll` ops in one
                // cascade section share the same span, so stop after the first.
                for op in &section.ops {
                    if let CascadeOp::Call(ident, _, args) = op
                        && ident.name == "addAll"
                        && args.positional.len() == 1
                        && args.named.is_empty()
                    {
                        self.push(&section.span);
                        break;
                    }
                }
            }
        }
        walk_expr(self, node);
    }
}

fn is_collection_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. }
    )
}
