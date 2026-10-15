//! Flags a redundant `.new` on constructor declarations and invocations.
//!
//! `X.new(...)` names the default (unnamed) constructor explicitly, which is exactly
//! equivalent to the shorter `X(...)`, so the `.new` is noise. The rule catches both
//! the plain call form (`X.new(...)`) and the `new`/`const` form (`const X.new(...)`).
//! Declarations such as `class X { X.new(); }` are also reported.
//! A bare `X.new` constructor tear-off that is not invoked is deliberately left alone:
//! there the `.new` is required to reference the constructor as a value.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UnnecessaryConstructorName;

impl Rule for UnnecessaryConstructorName {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    // `class A { A.new(); }` — declaring the default constructor as `.new`.
    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        if let Some(name) = &node.constructor_name
            && name.name == "new"
        {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-constructor-name",
                "Unnecessary `.new`; the default constructor can be invoked without it.",
                name.span,
            ));
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            // `X.new(...)` — the invocation form parses as a call whose callee is a
            // `.new` field access. The bare tear-off `X.new` is a `Field` not wrapped
            // in a `Call`, so it is not reached here.
            Expr::Call { callee, .. } => {
                if let Expr::Field { field, .. } = callee.unparenthesized()
                    && field.name == "new"
                {
                    self.diags.push(self.ctx.diagnostic(
                        "unnecessary-constructor-name",
                        "Unnecessary `.new`; the default constructor can be invoked without it.",
                        field.span,
                    ));
                }
            }
            // `new X.new(...)` / `const X.new(...)` — redundant named constructor.
            Expr::New {
                constructor_name: Some(name),
                ..
            } if name.name == "new" => {
                self.diags.push(self.ctx.diagnostic(
                    "unnecessary-constructor-name",
                    "Unnecessary `.new`; the default constructor can be invoked without it.",
                    name.span,
                ));
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
