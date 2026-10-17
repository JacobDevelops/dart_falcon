//! Flags `throw` inside a `finally` block.
//!
//! Throwing from a finally replaces whatever the try or catch was doing: an
//! exception that was still propagating is silently discarded and a value the
//! try was about to return is lost. Keep finally blocks limited to cleanup. A
//! throw inside a closure or local function defined in the finally is left
//! alone, as is a throw whose innermost enclosing `try` is nested inside the
//! finally (in its body or catch clauses).
//!
//! ```dart
//! try {
//!   work();
//! } finally {
//!   throw StateError('cleanup failed'); // flagged
//! }
//! ```
//!
//! Adopted from the official `throw_in_finally` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ThrowInFinally;

impl Rule for ThrowInFinally {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let finder = Finder {
            ctx,
            diagnostics: Vec::new(),
            in_finally: false,
            states: Vec::new(),
            finally_blocks: Vec::new(),
        };
        RulePreparation::Structural(Box::new(finder))
    }
}

/// Tracks whether the innermost enclosing `try` statement (within the current
/// function body) has the visited node inside its `finally` block.
struct Finder<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
    in_finally: bool,
    states: Vec<bool>,
    finally_blocks: Vec<Option<Span>>,
}

impl Finder<'_, '_> {
    fn report(&mut self, span: Span) {
        if self.in_finally {
            self.diagnostics.push(self.ctx.diagnostic(
                "throw-in-finally",
                "Use of 'throw' in a 'finally' block.",
                span,
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Finder<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.states.push(self.in_finally);
        match node {
            Stmt::TryCatch(try_catch) => {
                self.finally_blocks.push(try_catch.finally().map(|block| block.span()));
                self.in_finally = false;
            }
            Stmt::Throw(throw) => self.report(Span {
                start: throw.span().start,
                end: throw.value().span().end,
            }),
            Stmt::LocalFunc(_) => self.in_finally = false,
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::TryCatch(_)) { self.finally_blocks.pop(); }
        self.in_finally = self.states.pop().unwrap();
    }

    fn visit_block(&mut self, node: Block<'a>) -> StructuralTraversal {
        self.states.push(self.in_finally);
        if self.finally_blocks.last() == Some(&Some(node.span())) {
            self.in_finally = true;
        }
        StructuralTraversal::Continue
    }

    fn exit_block(&mut self, _node: Block<'a>) {
        self.in_finally = self.states.pop().unwrap();
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.states.push(self.in_finally);
        match node {
            Expr::Throw(throw) => self.report(throw.span()),
            Expr::FuncExpr(_) => self.in_finally = false,
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.in_finally = self.states.pop().unwrap();
    }
}
}
