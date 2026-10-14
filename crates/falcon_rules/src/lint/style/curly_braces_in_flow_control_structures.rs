//! Requires curly braces around the bodies of flow-control statements.
//!
//! A brace-less body invites the "goto fail" class of bug, where a later edit
//! adds a second statement that silently falls outside the branch. Requiring
//! blocks around `for`, `while`, `do`, and `if`/`else` bodies keeps the scope
//! explicit and edits safe. Two carve-outs match the official lint: an `if`
//! with no `else` whose body sits on the same line as its condition may omit
//! braces, and an `else if` chain need not wrap the intermediate `if` in a
//! block.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

pub struct CurlyBracesInFlowControlStructures;

impl Rule for CurlyBracesInFlowControlStructures {
    fn name(&self) -> &'static str {
        "curly-braces-in-flow-control-structures"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
    source: &'s str,
}

impl Collector<'_> {
    /// True when the body begins on the same source line as the condition's
    /// closing `)`, mirroring the official single-line `if` exemption.
    fn on_condition_line(&self, body: &Stmt) -> bool {
        let start = body.span().start;
        match self.source[..start].rfind(')') {
            Some(p) => !self.source[p..start].contains('\n'),
            None => true,
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::If(i) => match &i.else_branch {
                None => {
                    if !matches!(&*i.then_branch, Stmt::Block(_))
                        && !self.on_condition_line(&i.then_branch)
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "curly-braces-in-flow-control-structures",
                            "Use curly braces around the body of this flow-control statement",
                            *i.then_branch.span(),
                        ));
                    }
                }
                Some(eb) => {
                    if !matches!(&*i.then_branch, Stmt::Block(_)) {
                        self.diags.push(self.ctx.diagnostic(
                            "curly-braces-in-flow-control-structures",
                            "Use curly braces around the body of this flow-control statement",
                            *i.then_branch.span(),
                        ));
                    }
                    // An `else if` chain is fine; any other non-block else is not.
                    if !matches!(&**eb, Stmt::Block(_) | Stmt::If(_)) {
                        self.diags.push(self.ctx.diagnostic(
                            "curly-braces-in-flow-control-structures",
                            "Use curly braces around the body of this flow-control statement",
                            *eb.span(),
                        ));
                    }
                }
            },
            Stmt::For(f) if !matches!(&*f.body, Stmt::Block(_)) => {
                self.diags.push(self.ctx.diagnostic(
                    "curly-braces-in-flow-control-structures",
                    "Use curly braces around the body of this flow-control statement",
                    *f.body.span(),
                ))
            }
            Stmt::While(w) if !matches!(&*w.body, Stmt::Block(_)) => {
                self.diags.push(self.ctx.diagnostic(
                    "curly-braces-in-flow-control-structures",
                    "Use curly braces around the body of this flow-control statement",
                    *w.body.span(),
                ))
            }
            Stmt::DoWhile(d) if !matches!(&*d.body, Stmt::Block(_)) => {
                self.diags.push(self.ctx.diagnostic(
                    "curly-braces-in-flow-control-structures",
                    "Use curly braces around the body of this flow-control statement",
                    *d.body.span(),
                ))
            }
            _ => {}
        }
        walk_stmt(self, node);
    }
}
