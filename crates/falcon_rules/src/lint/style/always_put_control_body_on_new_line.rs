//! Flags a control-flow body that starts on the same line as its control clause.
//!
//! Putting the body of an `if`, `else`, `for`, `while`, or `do` on the line of
//! its condition hides it from a reader skimming the left edge of the code. For
//! a block body, the first statement inside the block is what must move:
//!
//! ```dart
//! if (done) return;            // flagged
//! while (x) { x = next(); }    // flagged
//! if (done) {
//!   return;
//! } else return;               // flagged
//! if (done) {}                 // fine: empty block
//! ```
//!
//! An `else if` chain is not reported at its `else`.
//!
//! Adopted from the official `always_put_control_body_on_new_line` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::token::Token;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct AlwaysPutControlBodyOnNewLine;

impl Rule for AlwaysPutControlBodyOnNewLine {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let tokens: Vec<Token> = ctx
            .tokens()
            .iter()
            .copied()
            .filter(|token| !token.is_trivia())
            .collect();
        let mut collector = Collector {
            ctx,
            tokens,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    /// Reports `body` when its first statement shares a line with the token
    /// that ends the control clause (`)`, `do`, or `else`).
    fn check(&mut self, body: &Stmt) {
        let first = match body {
            Stmt::Block(block) => match block.stmts.first() {
                Some(statement) => statement.span().start,
                None => return,
            },
            other => other.span().start,
        };
        let body_start = body.span().start;
        let before = self
            .tokens
            .partition_point(|token| token.offset() < body_start);
        let Some(control) = before.checked_sub(1).map(|index| self.tokens[index]) else {
            return;
        };
        let control_end = control.offset() + control.len();
        if self.ctx.source[control_end.min(first)..first].contains('\n') {
            return;
        }
        let begin_len = self
            .tokens
            .get(before)
            .filter(|token| token.offset() == body_start)
            .map_or(1, |token| token.len());
        self.diags.push(self.ctx.diagnostic(
            "always-put-control-body-on-new-line",
            "Statement should be on a separate line.",
            DiagSpan {
                start: body_start,
                end: body_start + begin_len,
            },
        ));
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::If(stmt) => {
                self.check(&stmt.then_branch);
                if let Some(else_branch) = &stmt.else_branch
                    && !matches!(**else_branch, Stmt::If(_))
                {
                    self.check(else_branch);
                }
            }
            Stmt::For(stmt) => self.check(&stmt.body),
            Stmt::While(stmt) => self.check(&stmt.body),
            Stmt::DoWhile(stmt) => self.check(&stmt.body),
            _ => {}
        }
        walk_stmt(self, node);
    }
}
