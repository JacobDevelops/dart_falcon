//! Flags adjacent string literals used as an element of a list or set literal.
//!
//! Adjacent strings concatenate, so a missing comma between two elements
//! silently merges them into one:
//!
//! ```dart
//! var list = <String>[
//!   'a'      // flagged: a comma is missing, the list has two elements
//!   'b',
//!   'c',
//! ];
//! var list = <String>['a' + 'b', 'c']; // fine: concatenation is explicit
//! ```
//!
//! Also checked: the body of a `for` element, the `then` element of an `if`
//! element without `else` (or the `else` element when present), and constant
//! string elements of a list pattern in a `switch` statement case. Map
//! literals and parenthesized elements are not checked.
//!
//! Adopted from the official `no_adjacent_strings_in_list` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct NoAdjacentStringsInList;

impl Rule for NoAdjacentStringsInList {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            nested: false,
            nested_stack: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    nested: bool,
    nested_stack: Vec<bool>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "no-adjacent-strings-in-list",
            "Don't use adjacent strings in a list literal.",
            span,
        ));
    }

    /// The span of an unparenthesized adjacent-strings element.
    fn adjacent_element(&self, element: &CollectionElement) -> Option<Span> {
        let CollectionElement::Expr(Expr::StringLit(string)) = element else {
            return None;
        };
        string.is_adjacent.then_some(string.span)
    }

    fn check(&mut self, element: &CollectionElement) {
        if let Some(span) = self.adjacent_element(element) {
            self.report(span);
        }
    }

    /// Checks only the eligible direct branch; shared callbacks handle descendants.
    fn check_branch(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                self.check(else_elem.as_deref().unwrap_or(then_elem));
            }
            CollectionElement::For { element, .. } => self.check(element),
            _ => {}
        }
    }

    fn check_list_pattern(&mut self, pattern: &Pattern) {
        let mut pattern = pattern;
        while let Pattern::ParenPattern { inner, .. } = pattern {
            pattern = inner;
        }
        let Pattern::List(list) = pattern else {
            return;
        };
        for element in &list.elements {
            let ListPatternElement::Pattern(element) = element else {
                continue;
            };
            let string = match element {
                Pattern::Literal(LiteralPattern {
                    value: LiteralPatternValue::String(string),
                    ..
                }) => string,
                Pattern::Const(ConstPattern {
                    value: ConstPatternValue::Expr(expr),
                    ..
                }) => match expr.as_ref() {
                    Expr::StringLit(string) => string,
                    _ => continue,
                },
                _ => continue,
            };
            if string.is_adjacent {
                self.report(string.span);
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.nested_stack.push(self.nested);
        self.nested = matches!(node, Expr::List { .. } | Expr::Set { .. });
        if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node {
            for element in elements {
                self.check(element);
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.nested = self.nested_stack.pop().unwrap();
    }

    fn visit_collection_element(&mut self, node: &CollectionElement) -> StructuralTraversal {
        self.nested_stack.push(self.nested);
        if self.nested {
            self.check_branch(node);
            self.nested = matches!(node, CollectionElement::If { .. } | CollectionElement::For { .. });
        }
        StructuralTraversal::Continue
    }

    fn exit_collection_element(&mut self, _node: &CollectionElement) {
        self.nested = self.nested_stack.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::Switch(switch) = node {
            for case in &switch.cases {
                for kind in &case.cases {
                    if let SwitchCaseKind::Pattern(pattern, _) = kind {
                        self.check_list_pattern(pattern);
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
