//! Flags the null-assertion operator `!`.
//!
//! `x!` asserts at runtime that `x` is non-null and throws if it is not,
//! trading a compile-time guarantee for a potential crash. Prefer explicit null
//! handling — `?.`, `??`, an `if (x != null)` promotion, or restructuring so
//! the value is non-nullable — over silencing the type system. An assertion on
//! a map index (`map[key]!`) is exempt, since `Map`'s index operator returns a
//! nullable value by design and the assertion is idiomatic there.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct AvoidNonNullAssertion;

impl Rule for AvoidNonNullAssertion {
    fn name(&self) -> &'static str {
        "avoid-non-null-assertion"
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

fn diag(ctx: &AnalyzeContext, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "avoid-non-null-assertion",
        Severity::Warning,
        "Avoid using the null assertion operator '!'",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        // dart_code_linter exempts the null-assertion on a map index
        // (`map[key]!`), since `Map`'s index operator returns a nullable value.
        // Without type resolution we conservatively exempt any index operand.
        if let Expr::NullAssert { operand, span } = node
            && !matches!(operand.as_ref(), Expr::Index { .. })
        {
            self.diags.push(diag(self.ctx, span));
        }
        walk_expr(self, node);
    }
}
