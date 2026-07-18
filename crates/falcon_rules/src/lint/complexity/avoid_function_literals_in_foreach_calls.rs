//! Flags `xs.forEach((e) { ... })` — a function literal passed to `forEach`,
//! which a `for`-in loop expresses more clearly. Tear-offs (`xs.forEach(print)`)
//! and null-aware `?.forEach` are left alone. Adopted from package:lints
//! `avoid_function_literals_in_foreach_calls`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
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
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "avoid-function-literals-in-foreach-calls",
            Severity::Warning,
            "Avoid using a function literal in a 'forEach' call; use a for-in loop instead.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
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
            && params.positional.len() == 1
            && params.optional_positional.is_empty()
            && params.named.is_empty()
        {
            self.push(&field.span);
        }
        walk_expr(self, node);
    }
}
