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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct EmptyCatches;

impl Rule for EmptyCatches {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

fn check_catch(catch: &CatchClause, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !catch.body.stmts.is_empty() {
        return;
    }
    // Exempt `catch (_) {}` / `on E catch (_) {}`.
    if catch.exception_var.as_ref().is_some_and(|v| v.name == "_") {
        return;
    }
    // Exempt an intentionally documented empty body.
    let syntax = ctx.syntax.syntax_for_span(catch.body.span);
    let open = syntax.token_range(catch.body.span).start;
    let Some(close_id) = syntax.closing_delimiter(open) else {
        return;
    };
    let close = &syntax.tokens()[close_id.index()];
    if syntax.tokens()[open.index() + 1..close_id.index()]
        .iter()
        .any(|token| token.kind.is_comment())
    {
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

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(statement) = node {
            for catch in &statement.catches {
                check_catch(catch, &mut self.diags, self.ctx);
            }
        }
        walk_stmt(self, node);
    }
}
