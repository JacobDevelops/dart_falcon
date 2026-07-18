//! Flags explicit `new` in instance-creation expressions. Ported from package:lints `unnecessary_new`.
//! The `new` keyword is always optional in Dart 2+, so every occurrence is redundant.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct UnnecessaryNew;

impl Rule for UnnecessaryNew {
    fn name(&self) -> &'static str {
        "unnecessary-new"
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
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        // Only explicit `new X(...)` is parsed as `Expr::New { is_const: false }`;
        // an implicit `X(...)` is an `Expr::Call`, and `const X(...)` sets is_const.
        if let Expr::New {
            is_const: false,
            span,
            ..
        } = node
        {
            self.diags.push(Diagnostic::new(
                "unnecessary-new",
                Severity::Warning,
                "Unnecessary `new` keyword; it is optional and can be removed.",
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
