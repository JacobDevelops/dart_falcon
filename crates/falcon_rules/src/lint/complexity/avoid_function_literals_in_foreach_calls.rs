//! Flags a function literal passed to `forEach`, e.g. `xs.forEach((e) { ... })`.
//!
//! A `for`-in loop reads more clearly than `forEach` with a closure, and unlike
//! the closure it can `await`, `break`, and `continue`, and it allocates no
//! per-call function object. The rule fires only for single-parameter
//! function-literal callbacks, so `Map.forEach` (which takes two parameters) is
//! left alone, as are tear-offs like `xs.forEach(print)` and null-aware
//! `xs?.forEach(...)`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct AvoidFunctionLiteralsInForeachCalls;

impl Rule for AvoidFunctionLiteralsInForeachCalls {
    fn name(&self) -> &'static str {
        "avoid-function-literals-in-foreach-calls"
    }

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

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, args, .. } = node
        && let Expr::Field {
            field,
            is_null_safe: false,
            ..
        } = &**callee
        && field.name == "forEach"
        && args.positional.len() == 1
        && args.named.is_empty()
        && let Expr::FuncExpr { params, .. } = &args.positional[0]
        // A single-parameter callback marks an Iterable.forEach; Map.forEach
        // takes two parameters and is intentionally left alone.
        && matches!(params.params.as_slice(), [param] if param.kind == falcon_syntax::ast::ParamKind::Positional)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-function-literals-in-foreach-calls",
                "Avoid using a function literal in a 'forEach' call; use a for-in loop instead.",
                field.span,
            ));
        }
        walk_expr(self, node);
    }
}
