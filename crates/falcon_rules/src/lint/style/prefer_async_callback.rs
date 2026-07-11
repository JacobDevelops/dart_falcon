//! Flags `Future<void> Function()` in favor of `AsyncCallback`. Ported from pyramid_lint's `prefer_async_callback`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_dart_type};

pub struct PreferAsyncCallback;

impl Rule for PreferAsyncCallback {
    fn name(&self) -> &'static str {
        "prefer_async_callback"
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
    fn visit_dart_type(&mut self, node: &DartType) {
        if is_future_void_function(node) {
            let span = node.span();
            self.diags.push(Diagnostic::new(
                "prefer_async_callback",
                Severity::Warning,
                "Use AsyncCallback instead of Future<void> Function().",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_dart_type(self, node);
    }
}

/// True for the type `Future<void> Function()` with no parameters.
fn is_future_void_function(t: &DartType) -> bool {
    let DartType::Function(ft) = t else {
        return false;
    };
    if !ft.params.is_empty() {
        return false;
    }
    let Some(ret) = &ft.return_type else {
        return false;
    };
    let DartType::Named(nt) = ret.as_ref() else {
        return false;
    };
    nt.segments.last().map(|s| s.name.as_str()) == Some("Future")
        && nt.type_args.len() == 1
        && matches!(nt.type_args[0], DartType::Void { .. })
}
