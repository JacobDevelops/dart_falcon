//! Flags an explicit `new` keyword in an instance-creation expression.
//!
//! Since Dart 2.0 the `new` keyword is always optional: `new Foo()` and `Foo()`
//! construct identically. Keeping `new` is pure noise and breaks visual
//! consistency with the surrounding code, which almost always omits it. The rule
//! only targets explicit `new`; an implicit `Foo()` parses as a call and a
//! `const Foo()` keeps its `const`, so neither is touched. Delete the `new`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::{Diagnostic, FixKind, TextEdit};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryNew;

impl Rule for UnnecessaryNew {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::New(expression) = node.unparenthesized()
            && !expression.is_const()
        {
            let diagnostic = self.ctx.diagnostic(
                "Remove the optional `new` keyword.",
                expression.span(),
            );
            let start = expression.span().start;
            let keyword_end = start + "new".len();
            let whitespace = self.ctx.source()[keyword_end..]
                .bytes()
                .take_while(|byte| matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
                .count();
            self.diags.push(self.ctx.with_fix(
                diagnostic,
                "Remove the optional `new` keyword",
                FixKind::Safe,
                vec![TextEdit::new(
                    falcon_diagnostics::Span {
                        start,
                        end: keyword_end + whitespace,
                    },
                    "",
                )],
            ));
        }
        StructuralTraversal::Continue
    }
}
}
