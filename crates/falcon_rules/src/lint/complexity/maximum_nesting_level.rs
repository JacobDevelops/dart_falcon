//! Flags a function whose control-flow nesting runs deeper than the configured level.
//!
//! Deeply nested blocks push logic to the right and make control flow hard to
//! follow; early returns, guard clauses, and extracted helpers flatten it. The
//! rule measures the deepest nesting of control structures — `if`, `for`,
//! `while`, `do`, `switch`, and `try` — within a function body. A plain block
//! does not add a level, `else if` chains stay at the same level, and a nested
//! local function or closure restarts the count. Their deepest level still
//! contributes to the enclosing declaration's maximum. Operator bodies are
//! included; the rule reports at the declaration when depth exceeds the threshold.
//!
//! ## Options
//!
//! `max_nesting` (integer, default: 5) — flag when the nesting depth exceeds
//! this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_declared_body;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body, walk_stmt};

#[derive(Default)]
pub struct MaximumNestingLevel;

impl Rule for MaximumNestingLevel {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_declared_body(program, ctx.source, &mut |body, name, _| {
            check_function(body, name, &mut diags, ctx);
        });
        diags
    }
}

/// Read the `max_nesting` option (default 5). Malformed/missing → default.
fn max_nesting_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "maximum-nesting-level"),
        "max_nesting",
    )
    .unwrap_or(5)
}

/// Compute the deepest control-flow nesting level within one function body and
/// emit a diagnostic at `name_span` when it exceeds the configured threshold.
///
/// Only control structures (if/for/while/do-while/switch/try) increase depth;
/// a plain block does not. Nested local functions start a fresh depth of 0.
fn check_function(
    body: &FunctionBody,
    name_span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let threshold = max_nesting_option(ctx);
    let mut nesting = Nesting {
        depth: 0,
        max: 0,
        else_if: None,
    };
    walk_function_body(&mut nesting, body);
    let max = nesting.max;
    if max > threshold {
        diags.push(ctx.diagnostic(
            "maximum-nesting-level",
            format!("Function has a nesting level of {max} (max {threshold})."),
            *name_span,
        ));
    }
}

struct Nesting {
    depth: usize,
    max: usize,
    else_if: Option<usize>,
}

impl Visitor for Nesting {
    fn visit_stmt(&mut self, node: &Stmt) {
        let outer = self.depth;
        let outer_else_if = self.else_if;
        if matches!(node, Stmt::LocalFunc(_)) {
            self.depth = 0;
        } else if matches!(
            node,
            Stmt::If(_)
                | Stmt::For(_)
                | Stmt::While(_)
                | Stmt::DoWhile(_)
                | Stmt::Switch(_)
                | Stmt::TryCatch(_)
        ) {
            if !matches!(node, Stmt::If(n) if self.else_if == Some(n.span.start)) {
                self.depth += 1;
            }
            self.max = self.max.max(self.depth);
        }
        if let Stmt::If(n) = node {
            // Else-if chains share the outer if's level.
            self.else_if = n.else_branch.as_deref().and_then(|branch| match branch {
                Stmt::If(n) => Some(n.span.start),
                _ => None,
            });
        }
        walk_stmt(self, node);
        self.else_if = outer_else_if;
        self.depth = outer;
    }

    fn visit_expr(&mut self, node: &Expr) {
        let outer = self.depth;
        if matches!(node, Expr::FuncExpr { .. }) {
            self.depth = 0;
        }
        walk_expr(self, node);
        self.depth = outer;
    }
}
