//! Flags a function whose cyclomatic complexity exceeds the configured threshold.
//!
//! Cyclomatic complexity counts the independent paths through a function; a high
//! value means many branches to understand and test. The rule computes
//! complexity as one plus each decision point — `if`, ternary, `&&`, `||`, `??`,
//! loops (`for`/`while`/`do`), `catch` clauses, non-default `case`s, and pattern
//! `when` guards — counted across the whole body tree, so decision points inside
//! nested closures and local functions count toward the enclosing function. It
//! reports at the function name when the total exceeds the threshold.
//! Operator bodies are included. Collection `if`/`for` elements are not decision
//! points themselves, but decisions in their conditions and children are counted.
//! Parameter defaults are constant expressions, not runtime body decisions, and
//! are excluded for both the declaration and its nested functions.
//!
//! ## Options
//!
//! `max_complexity` (integer, default: 20) — flag when the computed complexity
//! exceeds this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body, walk_stmt};

use falcon_syntax::visitor::for_each_declared_body;

#[derive(Default)]
pub struct CyclomaticComplexity;

impl Rule for CyclomaticComplexity {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_declared_body(program, ctx.source, &mut |body, name, _| {
            check_function(body, name, &mut diags, ctx);
        });
        diags
    }
}

/// Read the `max_complexity` option (default 20). Malformed/missing → default.
fn max_complexity_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "cyclomatic-complexity"),
        "max_complexity",
    )
    .unwrap_or(20)
}

/// Compute cyclomatic complexity for one function-like body and emit a
/// diagnostic at `name_span` when it exceeds the configured threshold.
///
/// Complexity = 1 (base) + number of decision points found *anywhere* in the
/// body tree. We take the simple "count everything in the body tree" approach:
/// decision points inside nested closures and local functions are counted
/// toward this enclosing function rather than being reported separately.
fn check_function(
    body: &FunctionBody,
    name_span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let threshold = max_complexity_option(ctx);
    let mut counter = DecisionPoints(0);
    walk_function_body(&mut counter, body);
    let complexity = counter.0 + 1;
    if complexity > threshold {
        diags.push(ctx.diagnostic(
            "cyclomatic-complexity",
            format!("Function has a cyclomatic complexity of {complexity} (max {threshold})."),
            *name_span,
        ));
    }
}

struct DecisionPoints(usize);

impl Visitor for DecisionPoints {
    fn visit_formal_param(&mut self, _node: &FormalParam) {
        // Constant defaults are not runtime paths through the enclosing body.
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        self.0 += match node {
            Stmt::If(n) => {
                1 + usize::from(matches!(&n.condition, IfCondition::Case(_, _, Some(_))))
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => 1,
            Stmt::TryCatch(n) => n.catches.len(),
            Stmt::Switch(n) => n
                .cases
                .iter()
                .flat_map(|case| &case.cases)
                .map(|kind| match kind {
                    SwitchCaseKind::Pattern(_, guard) => 1 + usize::from(guard.is_some()),
                    SwitchCaseKind::Expr(_) => 1,
                    SwitchCaseKind::Default => 0,
                })
                .sum(),
            _ => 0,
        };
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        self.0 += match node {
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce,
                ..
            }
            | Expr::Conditional { .. } => 1,
            Expr::Switch { arms, .. } => arms
                .iter()
                .map(|arm| 1 + usize::from(arm.guard.is_some()))
                .sum(),
            _ => 0,
        };
        walk_expr(self, node);
    }
}
