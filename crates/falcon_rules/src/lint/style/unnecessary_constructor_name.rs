//! Flags redundant `.new` on constructor invocations. Ported from package:lints `unnecessary_constructor_name`.
//! `X.new()` is equivalent to `X()`, so the explicit `.new` can be dropped. A bare `X.new`
//! tear-off expression (not invoked) is left alone — there the `.new` is required.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct UnnecessaryConstructorName;

impl Rule for UnnecessaryConstructorName {
    fn name(&self) -> &'static str {
        "unnecessary-constructor-name"
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
    fn flag(&mut self, span: &falcon_syntax::ast::Span) {
        self.diags.push(Diagnostic::new(
            "unnecessary-constructor-name",
            Severity::Warning,
            "Unnecessary `.new`; the default constructor can be invoked without it.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            // `X.new(...)` — the invocation form parses as a call whose callee is a
            // `.new` field access. The bare tear-off `X.new` is a `Field` not wrapped
            // in a `Call`, so it is not reached here.
            Expr::Call { callee, .. } => {
                if let Expr::Field { field, .. } = callee.as_ref()
                    && field.name == "new"
                {
                    self.flag(&field.span);
                }
            }
            // `new X.new(...)` / `const X.new(...)` — redundant named constructor.
            Expr::New {
                constructor_name: Some(name),
                ..
            } if name.name == "new" => {
                self.flag(&name.span);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
