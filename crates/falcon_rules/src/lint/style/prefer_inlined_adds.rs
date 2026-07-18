//! Flags an `add` cascade on a list literal whose element could be written inline.
//!
//! Building a list by literal-then-cascade (`[a]..add(b)`) is more verbose than
//! placing the element directly in the literal (`[a, b]`), and the inline form
//! reads as a single collection rather than a construction followed by mutation.
//! The rule matches each `..add(...)` section cascaded onto a list literal that
//! takes exactly one positional argument and no named arguments; multi-argument
//! or named calls, and cascades on non-list receivers, are ignored.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferInlinedAdds;

impl Rule for PreferInlinedAdds {
    fn name(&self) -> &'static str {
        "prefer-inlined-adds"
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
            "prefer-inlined-adds",
            Severity::Warning,
            "Inline the added element into the collection literal instead of using 'add'.",
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
            && matches!(&**object, Expr::List { .. })
        {
            for section in sections {
                // A section reports at most once: multiple `add` ops in one
                // cascade section share the same span, so stop after the first.
                for op in &section.ops {
                    if let CascadeOp::Call(ident, _, args) = op
                        && ident.name == "add"
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
