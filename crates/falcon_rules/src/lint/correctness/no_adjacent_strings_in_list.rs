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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoAdjacentStringsInList;

impl Rule for NoAdjacentStringsInList {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

impl<'a> Collector<'a> {
    fn report(ctx: &AnalyzeContext<'_>, diags: &mut Vec<Diagnostic>, span: Span) {
        diags.push(ctx.diagnostic("Don't use adjacent strings in a list literal.", span));
    }

    /// The span of an unparenthesized adjacent-strings element.
    fn adjacent_element(element: CollectionElement<'_>) -> Option<Span> {
        let CollectionElement::Expr(Expr::StringLit(string)) = element else {
            return None;
        };
        string.is_adjacent().then_some(string.span())
    }

    fn check(
        element: CollectionElement<'_>,
        ctx: &AnalyzeContext<'_>,
        diags: &mut Vec<Diagnostic>,
    ) {
        if let Some(span) = Self::adjacent_element(element) {
            Self::report(ctx, diags, span);
        }
    }
    /// Checks only the eligible direct branch; shared callbacks handle descendants.
    fn check_branch(
        element: CollectionElement<'_>,
        ctx: &AnalyzeContext<'_>,
        diags: &mut Vec<Diagnostic>,
    ) {
        match element {
            CollectionElement::If(node) => {
                Self::check(node.else_elem().unwrap_or_else(|| node.then_elem()), ctx, diags);
            }
            CollectionElement::For(node) => Self::check(node.element(), ctx, diags),
            _ => {}
        }
    }


    fn check_list_pattern(&mut self, pattern: Pattern<'a>) {
        let mut pattern = pattern;
        while let Pattern::ParenPattern(node) = pattern {
            pattern = node.inner();
        }
        let Pattern::List(list) = pattern else {
            return;
        };
        for element in list.elements() {
            let ListPatternElement::Pattern(element) = element else {
                continue;
            };
            let string = match element {
                Pattern::Literal(node) => match node.value() {
                    LiteralPatternValue::String(string) => string,
                    _ => continue,
                },
                Pattern::Const(node) => match node.value() {
                    ConstPatternValue::Expr(Expr::StringLit(string)) => string,
                    _ => continue,
                },
                _ => continue,
            };
            if string.is_adjacent() {
                Self::report(self.ctx, &mut self.diags, string.span());
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.nested_stack.push(self.nested);
        self.nested = matches!(node, Expr::List(_) | Expr::Set(_));
        let elements = match node {
            Expr::List(node) => Some(node.elements()),
            Expr::Set(node) => Some(node.elements()),
            _ => None,
        };
        if let Some(elements) = elements {
            for element in elements {
                Self::check(element, self.ctx, &mut self.diags);
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.nested = self.nested_stack.pop().unwrap();
    }

    fn visit_collection_element(&mut self, node: CollectionElement<'a>) -> StructuralTraversal {
        self.nested_stack.push(self.nested);
        if self.nested {
            Self::check_branch(node, self.ctx, &mut self.diags);
            self.nested = matches!(node, CollectionElement::If(_) | CollectionElement::For(_));
        }
        StructuralTraversal::Continue
    }

    fn exit_collection_element(&mut self, _node: CollectionElement<'a>) {
        self.nested = self.nested_stack.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Switch(switch) = node {
            for case in switch.cases() {
                for kind in case.cases() {
                    if let SwitchCaseKind::Pattern(label) = kind {
                        self.check_list_pattern(label.pattern());
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
