//! Flags a ternary conditional expression nested inside another ternary.
//!
//! Stacked conditionals like `a ? b : c ? d : e` are hard to read and easy to
//! misparse; an `if`/`else` chain, a `switch` expression, or an extracted
//! variable makes the branching explicit. The rule reports any conditional
//! expression that has another conditional as an ancestor (nesting level two or
//! deeper), matching dart_code_linter's default acceptable level of one.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct AvoidNestedConditionalExpressions;

impl Rule for AvoidNestedConditionalExpressions {
    fn name(&self) -> &'static str {
        "avoid-nested-conditional-expressions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            cond_ancestor: false,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    cond_ancestor: bool,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { .. } = node {
            // A closure body is read on its own; a ternary inside one is not
            // stacked with the ternary the closure happens to sit in.
            let saved = std::mem::replace(&mut self.cond_ancestor, false);
            walk_expr(self, node);
            self.cond_ancestor = saved;
            return;
        }
        if let Expr::Conditional { span, .. } = node {
            if self.cond_ancestor {
                self.diags.push(Diagnostic::new(
                    "avoid-nested-conditional-expressions",
                    Severity::Warning,
                    "Avoid nested conditional expressions",
                    self.ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
            // Everything under this conditional now has a conditional ancestor.
            let saved = std::mem::replace(&mut self.cond_ancestor, true);
            walk_expr(self, node);
            self.cond_ancestor = saved;
            return;
        }
        walk_expr(self, node);
    }
}
