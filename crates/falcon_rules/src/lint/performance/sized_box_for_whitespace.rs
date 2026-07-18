//! Flags a `Container` used only for `width`/`height` whitespace. Ported from package:lints `sized_box_for_whitespace`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct SizedBoxForWhitespace;

impl Rule for SizedBoxForWhitespace {
    fn name(&self) -> &'static str {
        "sized-box-for-whitespace"
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

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((name, args, span)) = widget_construction(node)
            && name == "Container"
            && args.positional.is_empty()
            // Only sizing/pass-through args, and at least one dimension set: this
            // Container adds whitespace only, so a SizedBox expresses it directly.
            && args
                .named
                .iter()
                .any(|n| matches!(n.name.name.as_str(), "width" | "height"))
            && args
                .named
                .iter()
                .all(|n| matches!(n.name.name.as_str(), "width" | "height" | "child" | "key"))
        {
            self.diags.push(Diagnostic::new(
                "sized-box-for-whitespace",
                Severity::Warning,
                "Use a SizedBox to add whitespace to a layout instead of a Container.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}

/// Resolve a widget construction expressed as either an implicit call
/// (`Container(...)`) or an explicit `new`/`const` (`Container(...)`).
fn widget_construction(expr: &Expr) -> Option<(&str, &ArgList, &Span)> {
    match expr {
        Expr::New {
            dart_type: DartType::Named(nt),
            args,
            span,
            ..
        } => nt.segments.last().map(|s| (s.name.as_str(), args, span)),
        Expr::Call {
            callee, args, span, ..
        } => {
            if let Expr::Ident(id) = callee.as_ref() {
                Some((id.name.as_str(), args, span))
            } else {
                None
            }
        }
        _ => None,
    }
}
