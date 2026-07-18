//! Flags a redundant `.new` on constructor invocations.
//!
//! `X.new(...)` names the default (unnamed) constructor explicitly, which is exactly
//! equivalent to the shorter `X(...)`, so the `.new` is noise. The rule catches both
//! the plain call form (`X.new(...)`) and the `new`/`const` form (`const X.new(...)`).
//! A bare `X.new` constructor tear-off that is not invoked is deliberately left alone:
//! there the `.new` is required to reference the constructor as a value.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_constructor_decl, walk_expr, walk_program};

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

    // `class A { A.new(); }` — declaring the default constructor as `.new`.
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        if let Some(name) = &node.constructor_name
            && name.name == "new"
        {
            self.flag(&name.span);
        }
        walk_constructor_decl(self, node);
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
