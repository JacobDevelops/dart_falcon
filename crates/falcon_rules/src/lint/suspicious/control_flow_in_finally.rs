//! Flags `return`, `break`, or `continue` that escapes a `finally` block.
//!
//! Control flow leaving a finally overrides whatever the try or catch was doing,
//! including silently discarding an exception that was still propagating — the
//! error simply vanishes — and masking any value the try was about to return.
//! Keep finally blocks limited to cleanup and let exceptions and returns flow
//! through. A `break` or `continue` targeting a loop or switch declared *inside*
//! the finally is fine and left alone, as are closures defined within it; only
//! flow that escapes the finally itself is reported.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ControlFlowInFinally;

impl Rule for ControlFlowInFinally {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let finder = Finder {
            diags: Vec::new(),
            ctx,
            contexts: Vec::new(),
            finally_blocks: Vec::new(),
            floor: 0,
            floors: Vec::new(),
        };
        RulePreparation::Structural(Box::new(finder))
    }
}

struct Finder<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    contexts: Vec<FinallyContext<'ctx>>,
    finally_blocks: Vec<Option<Span>>,
    floor: usize,
    floors: Vec<usize>,
}

#[derive(Default)]
struct FinallyContext<'a> {
    breakable: usize,
    loops: usize,
    break_labels: HashSet<&'a str>,
    loop_labels: HashSet<&'a str>,
}

impl<'a> FinallyContext<'a> {
    fn label_is_loop(labeled: LabeledStmt<'a>) -> bool {
        let mut target = labeled.stmt();
        while let Stmt::Labeled(inner) = target {
            target = inner.stmt();
        }
        matches!(target, Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_))
    }

    fn enter(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => {
                self.breakable += 1;
                self.loops += 1;
            }
            Stmt::Switch(switch) => {
                self.breakable += 1;
                for label in switch.cases().flat_map(SwitchCase::labels) {
                    self.loop_labels.insert(label.name());
                }
            }
            Stmt::Labeled(labeled) => {
                self.break_labels.insert(labeled.label().name());
                if Self::label_is_loop(labeled) {
                    self.loop_labels.insert(labeled.label().name());
                }
            }
            _ => {}
        }
    }

    fn exit(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => {
                self.breakable -= 1;
                self.loops -= 1;
            }
            Stmt::Switch(switch) => {
                self.breakable -= 1;
                for label in switch.cases().flat_map(SwitchCase::labels) {
                    self.loop_labels.remove(label.name());
                }
            }
            Stmt::Labeled(labeled) => {
                self.break_labels.remove(labeled.label().name());
                self.loop_labels.remove(labeled.label().name());
            }
            _ => {}
        }
    }

    fn escaping_span(&self, node: Stmt<'a>) -> Option<Span> {
        match node {
            Stmt::Return(ret) => Some(ret.span()),
            Stmt::Break(brk)
                if !brk.label().map_or(self.breakable > 0, |label| {
                    self.break_labels.contains(label.name())
                }) =>
            {
                Some(brk.span())
            }
            Stmt::Continue(cont)
                if !cont.label().map_or(self.loops > 0, |label| {
                    self.loop_labels.contains(label.name())
                }) =>
            {
                Some(cont.span())
            }
            _ => None,
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Finder<'_, 'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.diags.sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        self.diags.dedup_by_key(|diagnostic| diagnostic.span.start);
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.floors.push(self.floor);
        if matches!(node, Stmt::LocalFunc(_)) { self.floor = self.contexts.len(); }
        if let Stmt::TryCatch(try_catch) = node {
            self.finally_blocks.push(try_catch.finally().map(Block::span));
        }
        for context in &mut self.contexts[self.floor..] {
            if let Some(span) = context.escaping_span(node) {
                self.diags.push(self.ctx.diagnostic(
                    "Avoid control flow (return/break/continue) that escapes a finally block.",
                    span,
                ));
            }
            context.enter(node);
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        for context in &mut self.contexts[self.floor..] { context.exit(node); }
        if matches!(node, Stmt::TryCatch(_)) { self.finally_blocks.pop(); }
        self.floor = self.floors.pop().unwrap();
    }

    fn visit_block(&mut self, node: Block<'a>) -> StructuralTraversal {
        if self.finally_blocks.last() == Some(&Some(node.span())) {
            self.contexts.push(FinallyContext::default());
        }
        StructuralTraversal::Continue
    }

    fn exit_block(&mut self, node: Block<'a>) {
        if self.finally_blocks.last() == Some(&Some(node.span())) { self.contexts.pop(); }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.floors.push(self.floor);
        if matches!(node, Expr::FuncExpr(_)) { self.floor = self.contexts.len(); }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.floor = self.floors.pop().unwrap();
    }
}
}
