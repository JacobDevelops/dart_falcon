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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct NoAdjacentStringsInList;

impl Rule for NoAdjacentStringsInList {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
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

    /// Checks `for` / `if` element bodies at any depth below a list or set.
    fn check_nested(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                match else_elem {
                    None => self.check(then_elem),
                    Some(else_elem) => self.check(else_elem),
                }
                self.check_nested(then_elem);
                if let Some(else_elem) = else_elem {
                    self.check_nested(else_elem);
                }
            }
            CollectionElement::For { element, .. } => {
                self.check(element);
                self.check_nested(element);
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
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

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node {
            for element in elements {
                self.check(element);
                self.check_nested(element);
            }
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Switch(switch) = node {
            for case in &switch.cases {
                for kind in &case.cases {
                    if let SwitchCaseKind::Pattern(pattern, _) = kind {
                        self.check_list_pattern(pattern);
                    }
                }
            }
        }
        walk_stmt(self, node);
    }
}
