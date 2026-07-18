//! Flags a `Container` whose only argument is `child`. Ported from package:lints `avoid_unnecessary_containers`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct AvoidUnnecessaryContainers;

impl Rule for AvoidUnnecessaryContainers {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-containers"
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
            && args.named.len() == 1
            && args.named[0].name.name == "child"
        {
            self.diags.push(Diagnostic::new(
                "avoid-unnecessary-containers",
                Severity::Warning,
                "Avoid a Container with only a child; use the child directly.",
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
