//! Flags empty `catch` blocks.
//!
//! A catch clause with no body silently swallows the exception, hiding failures
//! that should be handled, logged, or rethrown and making bugs far harder to
//! diagnose. Handle the error, or at minimum log it, rather than discarding it.
//! Two escape hatches match the official lint: a catch whose body contains a
//! comment (the emptiness is deliberate and documented), and a catch that binds
//! its exception to `_`, an explicit "I am ignoring this" marker.
//!
//! This includes catch clauses inside anonymous functions.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct EmptyCatches;

impl Rule for EmptyCatches {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

fn check_catch(catch: CatchClause<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !catch.body().stmts().is_empty() {
        return;
    }
    // Exempt `catch (_) {}` / `on E catch (_) {}`.
    if catch.exception_var().is_some_and(|v| v.name() == "_") {
        return;
    }
    // Exempt an intentionally documented empty body.
    let mut tokens = catch.body().syntax().tokens();
    let Some(open) = tokens.next() else { return };
    let Some(close) = tokens.next_back() else {
        return;
    };
    if open.kind() != falcon_syntax::TokenKind::LBrace
        || close.kind() != falcon_syntax::TokenKind::RBrace
    {
        return;
    }
    if tokens.any(|token| token.kind().is_comment()) {
        return;
    }
    diags.push(ctx.diagnostic(
        "empty-catches",
        "Empty catch block — handle the exception, add a comment, or name it `_`",
        Span::new(close.offset(), close.offset() + close.len()),
    ));
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::TryCatch(statement) = node {
            for catch in statement.catches() {
                check_catch(catch, &mut self.diags, self.ctx);
            }
        }
        StructuralTraversal::Continue
    }
}
}
