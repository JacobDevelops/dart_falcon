//! Flags constructors with an empty block body `{}`.
//!
//! An empty `{}` is indistinguishable from a body someone forgot to fill in,
//! whereas a bare `;` states plainly that the constructor does no extra work.
//! Prefer `Foo();` over `Foo() {}`. Only a block body containing no statements
//! is reported; a constructor whose body does something is left alone.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct EmptyConstructorBodies;

impl Rule for EmptyConstructorBodies {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let Some(members) = crate::lint::declarations::members_of(decl) else {
            return StructuralTraversal::SkipChildren;
        };
        for member in members {
            if let ClassMember::Constructor(c) = member {
                check_constructor(c, &mut self.diags, self.ctx);
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn check_constructor(ctor: ConstructorDecl<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let FunctionBody::Block(block) = (match ctor.body() {
        Some(b) => b,
        None => return,
    }) else {
        return;
    };
    if !block.stmts().is_empty() {
        return;
    }
    diags.push(ctx.diagnostic(
        "empty-constructor-bodies",
        "Empty constructor body — use `;` instead of `{}`",
        DiagSpan {
            start: block.span().start,
            end: block.span().start + 1,
        },
    ));
}
