//! Flags `throw` statements inside a `catch` block.
//!
//! Throwing a fresh exception from a catch replaces the error in flight along
//! with its original stack trace, so the root cause is lost and the failure
//! becomes much harder to diagnose. Prefer `rethrow` to propagate the caught
//! exception unchanged, or wrap it in an error that keeps the original as a
//! cause. Every `throw` statement is reported, including throws of existing
//! values; `rethrow` is always exempt. Delayed local functions and closures are
//! separate execution scopes and are not part of the surrounding catch.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidThrowInCatchBlock;

impl Rule for AvoidThrowInCatchBlock {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            next_group: 0,
            try_groups: Vec::new(),
            active_group: None,
            scopes: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<(usize, Diagnostic)>,
    ctx: &'a AnalyzeContext<'ctx>,
    next_group: usize,
    try_groups: Vec<usize>,
    active_group: Option<usize>,
    scopes: Vec<Option<usize>>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, 'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        // The former catch scans ran before descending into each try statement.
        self.diags.sort_unstable_by_key(|(group, diagnostic)| (*group, diagnostic.span.start));
        self.diags.into_iter().map(|(_, diagnostic)| diagnostic).collect()
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        match node {
            Stmt::TryCatch(try_catch) => {
                self.try_groups.push(self.next_group);
                self.next_group += try_catch.catches().count();
            }
            Stmt::LocalFunc(_) => {
                self.scopes.push(self.active_group.take());
            }
            Stmt::Throw(throw_stmt) => {
                if let Some(group) = self.active_group {
                    self.diags.push((group, self.ctx.diagnostic(
                        "avoid-throw-in-catch-block",
                        "Avoid throwing exceptions within catch blocks",
                        throw_stmt.span(),
                    )));
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::TryCatch(_) => {
                self.try_groups.pop();
            }
            Stmt::LocalFunc(_) => {
                self.active_group = self.scopes.pop().unwrap();
            }
            _ => {}
        }
    }

    fn visit_catch_clause(&mut self, _node: CatchClause<'a>) -> StructuralTraversal {
        self.scopes.push(self.active_group);
        let group = self.try_groups.last_mut().unwrap();
        self.active_group = Some(*group);
        *group += 1;
        StructuralTraversal::Continue
    }

    fn exit_catch_clause(&mut self, _node: CatchClause<'a>) {
        self.active_group = self.scopes.pop().unwrap();
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr(_)) {
            self.scopes.push(self.active_group.take());
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.active_group = self.scopes.pop().unwrap();
        }
    }
}
}

#[cfg(test)]
mod tests {
    use super::AvoidThrowInCatchBlock;
    use falcon_analyze::{AnalyzeContext, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn preserves_catch_scan_order_across_nested_and_delayed_scopes() {
        let source =
            include_str!("../../../tests/corpus/avoid-throw-in-catch-block/bad_traversal.dart");
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parse(source).tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("catch_order.dart"), &config, &parsed);
        let actual: Vec<_> = AvoidThrowInCatchBlock
            .analyze(parsed.program(), &ctx)
            .into_iter()
            .map(|diagnostic| diagnostic.span.start)
            .collect();
        let expected: Vec<_> = [
            "throw StateError('$error')",
            "throw StateError('nested try')",
            "throw StateError('nested finally')",
            "throw StateError('$nested')",
            "throw StateError('outer restored')",
            "throw StateError('local catch')",
            "throw StateError('closure catch')",
        ]
        .into_iter()
        .map(|statement| source.find(statement).unwrap())
        .collect();
        assert_eq!(actual, expected);
    }
}
