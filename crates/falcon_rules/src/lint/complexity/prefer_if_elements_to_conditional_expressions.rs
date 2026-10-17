//! Prefers collection `if` elements to conditional expressions in list and set literals.
//!
//! Since Dart 2.3 a list or set literal can choose an element with an `if`
//! element, so `[a, b ? c : d]` reads better as `[a, if (b) c else d]`. The rule
//! flags a conditional expression that is itself a list or set element, or the
//! `then`/`else` element of a collection `if`, or the body of a collection `for`.
//! Conditionals nested deeper inside an element (`[f(b ? c : d)]`) and map
//! entries are left alone, as the official lint does.
//!
//! Adopted from the official `prefer_if_elements_to_conditional_expressions` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferIfElementsToConditionalExpressions;

impl Rule for PreferIfElementsToConditionalExpressions {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            groups: Vec::new(),
            group: None,
            expressions: Vec::new(),
            branches: Vec::new(),
        }))
    }
}

struct Branch {
    else_elem: *const CollectionElement,
    start: usize,
    else_start: usize,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    // The legacy visitor checks an entire literal before descending into nested literals.
    groups: Vec<Vec<Diagnostic>>,
    group: Option<usize>,
    expressions: Vec<Option<usize>>,
    branches: Vec<Branch>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.groups.into_iter().flatten().collect()
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.expressions.push(self.group);
        self.group = if matches!(node, Expr::List { .. } | Expr::Set { .. }) {
            let group = self.groups.len();
            self.groups.push(Vec::new());
            Some(group)
        } else {
            // Only collection if/for elements propagate eligibility, not expressions
            // (including map literals) nested inside a list or set element.
            None
        };
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.group = self.expressions.pop().unwrap();
    }

    fn visit_collection_element(&mut self, node: &CollectionElement) -> StructuralTraversal {
        let Some(group) = self.group else {
            return StructuralTraversal::Continue;
        };
        let diagnostics = &mut self.groups[group];
        if let Some(branch) = self.branches.last_mut()
            && std::ptr::eq(node, branch.else_elem)
        {
            branch.else_start = diagnostics.len();
        }
        match node {
            CollectionElement::If { else_elem: Some(else_elem), .. } => {
                self.branches.push(Branch {
                    else_elem: else_elem.as_ref(),
                    start: diagnostics.len(),
                    else_start: diagnostics.len(),
                });
            }
            CollectionElement::Expr(expr)
                if matches!(expr.unparenthesized(), Expr::Conditional { .. }) =>
            {
                diagnostics.push(self.ctx.diagnostic(
                    "prefer-if-elements-to-conditional-expressions",
                    "Use an 'if' element to conditionally add elements.",
                    *expr.unparenthesized().span(),
                ));
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_collection_element(&mut self, node: &CollectionElement) {
        if let Some(group) = self.group
            && matches!(node, CollectionElement::If { else_elem: Some(_), .. })
        {
            let branch = self.branches.pop().unwrap();
            // The legacy pending stack visits else before then; structural traversal
            // visits then first. Nested literals have separate buffers and stay put.
            self.groups[group][branch.start..].rotate_left(branch.else_start - branch.start);
        }
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn collection_branches_precede_nested_literal_diagnostics() {
        let source = "var values = [if (a) b ? 1 : 2 else c ? 3 : 4, [d ? 5 : 6], e ? 7 : 8];";
        assert_diagnostic_order(
            source,
            &["c ? 3 : 4", "b ? 1 : 2", "e ? 7 : 8", "d ? 5 : 6"],
        );
    }

    #[test]
    fn nested_else_if_branches_keep_legacy_order() {
        let source = "var values = [if (a) b ? 1 : 2 else if (c) d ? 3 : 4 else e ? 5 : 6, [f ? 7 : 8], g ? 9 : 10];";
        assert_diagnostic_order(
            source,
            &[
                "e ? 5 : 6",
                "d ? 3 : 4",
                "b ? 1 : 2",
                "g ? 9 : 10",
                "f ? 7 : 8",
            ],
        );
    }

    #[test]
    fn if_inside_for_keeps_legacy_order() {
        let source = "var result = [for (final value in values) if (a) b ? 1 : 2 else c ? 3 : 4, [d ? 5 : 6], e ? 7 : 8];";
        assert_diagnostic_order(
            source,
            &["c ? 3 : 4", "b ? 1 : 2", "e ? 7 : 8", "d ? 5 : 6"],
        );
    }

    fn assert_diagnostic_order(source: &str, expected: &[&str]) {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let diagnostics = PreferIfElementsToConditionalExpressions.analyze(&parsed.program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, expected);
    }
}
