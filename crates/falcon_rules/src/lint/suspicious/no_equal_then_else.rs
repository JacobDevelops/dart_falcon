//! Flags an `if`/`else` or ternary whose two branches are identical.
//!
//! When the then and else branches have the same body, the condition decides
//! nothing — the same code runs either way — so either the condition is pointless
//! or one branch was never edited to differ. Comparison is on branch source text
//! with whitespace and block comments normalized away, so formatting differences
//! do not hide a match. Remove the condition and keep the single body, or correct
//! the branch that was supposed to differ. Applies to both `if`/`else` statements
//! and `?:` ternary expressions.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program, walk_stmt};

pub struct NoEqualThenElse;

impl Rule for NoEqualThenElse {
    fn name(&self) -> &'static str {
        "no-equal-then-else"
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

// Strip /* ... */ block comments for comparison
fn strip_block_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn normalize(s: &str) -> String {
    strip_block_comments(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn span_src<'a>(source: &'a str, span: &Span) -> &'a str {
    let end = span.end.min(source.len());
    &source[span.start..end]
}

fn last_stmt_span(stmt: &Stmt) -> &Span {
    match stmt {
        Stmt::Block(b) => b.stmts.last().map(last_stmt_span).unwrap_or(&b.span),
        other => other.span(),
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Conditional {
            then_expr,
            else_expr,
            span,
            ..
        } = node
            && normalize(span_src(self.ctx.source, then_expr.span()))
                == normalize(span_src(self.ctx.source, else_expr.span()))
        {
            self.diags.push(Diagnostic::new(
                "no-equal-then-else",
                Severity::Warning,
                "Both branches of ternary are identical — remove the condition",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(i) = node
            && let Some(else_branch) = &i.else_branch
            && normalize(span_src(self.ctx.source, i.then_branch.span()))
                == normalize(span_src(self.ctx.source, else_branch.span()))
        {
            let last = last_stmt_span(&i.then_branch);
            self.diags.push(Diagnostic::new(
                "no-equal-then-else",
                Severity::Warning,
                "Both branches of if/else are identical — remove the condition",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: last.start,
                    end: last.end,
                },
            ));
        }
        walk_stmt(self, node);
    }
}
