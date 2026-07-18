//! Requires braces around flow-control bodies, ported from package:lints
//! `curly_braces_in_flow_control_structures`. `for`, `while` and `do` bodies
//! must always be blocks. `if`/`else` branches must be blocks too, with two
//! carve-outs matching the official lint: an `if` without an `else` whose
//! statement sits on the same line as its condition is allowed to omit braces,
//! and an `else if` chain does not require the intermediate `if` to be a block.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
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
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    file: String,
    source: &'s str,
}

impl Collector<'_> {
    fn report(&mut self, stmt: &Stmt) {
        let span = stmt.span();
        self.diags.push(Diagnostic::new(
            "curly-braces-in-flow-control-structures",
            Severity::Warning,
            "Use curly braces around the body of this flow-control statement",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

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
                        self.report(&i.then_branch);
                    }
                }
                Some(eb) => {
                    if !matches!(&*i.then_branch, Stmt::Block(_)) {
                        self.report(&i.then_branch);
                    }
                    // An `else if` chain is fine; any other non-block else is not.
                    if !matches!(&**eb, Stmt::Block(_) | Stmt::If(_)) {
                        self.report(eb);
                    }
                }
            },
            Stmt::For(f) if !matches!(&*f.body, Stmt::Block(_)) => self.report(&f.body),
            Stmt::While(w) if !matches!(&*w.body, Stmt::Block(_)) => self.report(&w.body),
            Stmt::DoWhile(d) if !matches!(&*d.body, Stmt::Block(_)) => self.report(&d.body),
            _ => {}
        }
        walk_stmt(self, node);
    }
}
