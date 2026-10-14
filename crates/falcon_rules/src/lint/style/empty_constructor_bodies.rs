//! Flags constructors with an empty block body `{}`.
//!
//! An empty `{}` is indistinguishable from a body someone forgot to fill in,
//! whereas a bare `;` states plainly that the constructor does no extra work.
//! Prefer `Foo();` over `Foo() {}`. Only a block body containing no statements
//! is reported; a constructor whose body does something is left alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct EmptyConstructorBodies;

impl Rule for EmptyConstructorBodies {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            for member in members {
                if let ClassMember::Constructor(c) = member {
                    check_constructor(c, &mut diags, ctx);
                }
            }
        }
        diags
    }
}

fn check_constructor(ctor: &ConstructorDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let FunctionBody::Block(block) = (match &ctor.body {
        Some(b) => b,
        None => return,
    }) else {
        return;
    };
    if !block.stmts.is_empty() {
        return;
    }
    diags.push(ctx.diagnostic(
        "empty-constructor-bodies",
        "Empty constructor body — use `;` instead of `{}`",
        DiagSpan {
            start: block.span.start,
            end: block.span.start + 1,
        },
    ));
}
