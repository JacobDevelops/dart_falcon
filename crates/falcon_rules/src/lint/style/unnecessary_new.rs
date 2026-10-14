//! Flags an explicit `new` keyword in an instance-creation expression.
//!
//! Since Dart 2.0 the `new` keyword is always optional: `new Foo()` and `Foo()`
//! construct identically. Keeping `new` is pure noise and breaks visual
//! consistency with the surrounding code, which almost always omits it. The rule
//! only targets explicit `new`; an implicit `Foo()` parses as a call and a
//! `const Foo()` keeps its `const`, so neither is touched. Delete the `new`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UnnecessaryNew;

impl Rule for UnnecessaryNew {
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
    fn visit_expr(&mut self, node: &Expr) {
        // Only explicit `new X(...)` is parsed as `Expr::New { is_const: false }`;
        // an implicit `X(...)` is an `Expr::Call`, and `const X(...)` sets is_const.
        if let Expr::New {
            is_const: false,
            span,
            ..
        } = node
        {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-new",
                "Unnecessary `new` keyword; it is optional and can be removed.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}
