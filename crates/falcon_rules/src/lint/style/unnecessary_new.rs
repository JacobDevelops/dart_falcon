//! Flags an explicit `new` keyword in an instance-creation expression.
//!
//! Since Dart 2.0 the `new` keyword is always optional: `new Foo()` and `Foo()`
//! construct identically. Keeping `new` is pure noise and breaks visual
//! consistency with the surrounding code, which almost always omits it. The rule
//! only targets explicit `new`; an implicit `Foo()` parses as a call and a
//! `const Foo()` keeps its `const`, so neither is touched. Delete the `new`.

use super::traversal::for_each_expr;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryNew;

impl Rule for UnnecessaryNew {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_expr(program, |node| {
            if let Expr::New {
                is_const: false,
                span,
                ..
            } = node
            {
                diags.push(ctx.diagnostic(
                    "unnecessary-new",
                    "Unnecessary `new` keyword; it is optional and can be removed.",
                    *span,
                ));
            }
        });
        diags
    }
}
