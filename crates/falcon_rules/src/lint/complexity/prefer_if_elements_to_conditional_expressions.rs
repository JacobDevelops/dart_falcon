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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct PreferIfElementsToConditionalExpressions;

impl Rule for PreferIfElementsToConditionalExpressions {
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
    fn check_element(&mut self, element: &CollectionElement) {
        let mut pending = vec![element];
        while let Some(element) = pending.pop() {
            match element {
                CollectionElement::Expr(expr)
                    if matches!(expr.unparenthesized(), Expr::Conditional { .. }) =>
                {
                    let span = expr.unparenthesized().span();
                    self.diags.push(self.ctx.diagnostic(
                        "prefer-if-elements-to-conditional-expressions",
                        "Use an 'if' element to conditionally add elements.",
                        *span,
                    ));
                }
                CollectionElement::If {
                    then_elem,
                    else_elem,
                    ..
                } => {
                    pending.push(then_elem);
                    pending.extend(else_elem.as_deref());
                }
                CollectionElement::For { element, .. } => pending.push(element),
                CollectionElement::Expr(_)
                | CollectionElement::NullAware { .. }
                | CollectionElement::Spread { .. } => {}
            }
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node {
            for element in elements {
                self.check_element(element);
            }
        }
        walk_expr(self, node);
    }
}
