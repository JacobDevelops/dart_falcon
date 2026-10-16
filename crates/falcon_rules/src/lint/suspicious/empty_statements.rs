//! Flags stray empty statements (a lone `;`).
//!
//! A solitary semicolon does nothing, and where it slips in it usually changes
//! meaning: `while (cond);` turns the following block into an unconditional body,
//! and a stray `;` after an `if` header detaches the intended branch. It is
//! almost always an accidental extra semicolon or a loop body that should have
//! been a block. Remove it, or replace it with the block that was intended. The
//! semicolons in a `for (;;)` header are loop syntax, not statements, and are
//! never reported. A real `null;` expression statement is not an empty statement.
//!
//! This includes empty statements inside anonymous functions.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct EmptyStatements;

impl Rule for EmptyStatements {
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

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Empty(span) = node {
            self.diags.push(self.ctx.diagnostic(
                "empty-statements",
                "Unnecessary empty statement — remove the `;` or replace it with a block",
                *span,
            ));
        }
        walk_stmt(self, node);
    }
}
