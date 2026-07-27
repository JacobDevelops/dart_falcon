//! Flags an `if`/`else` or ternary whose two branches are identical.
//!
//! When the then and else branches have the same body, the condition decides
//! nothing — the same code runs either way — so either the condition is pointless
//! or one branch was never edited to differ. Comparison is on branch source text
//! with whitespace outside strings and comments normalized away, so formatting
//! differences do not hide a match while literal and comment contents stay intact. Remove the condition and keep the single body, or correct
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

fn normalize(source: &str) -> Vec<u8> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Code,
        String,
        LineComment,
        BlockComment,
    }

    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut mode = Mode::Code;
    let mut quote = b'\0';
    let mut triple = false;
    let mut raw_string = false;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        match mode {
            Mode::Code if byte.is_ascii_whitespace() => index += 1,
            Mode::Code if byte == b'/' && next == Some(b'/') => {
                output.extend_from_slice(b"//");
                mode = Mode::LineComment;
                index += 2;
            }
            Mode::Code if bytes[index..].starts_with(b"/* expect:") => {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            Mode::Code if byte == b'/' && next == Some(b'*') => {
                output.extend_from_slice(b"/*");
                mode = Mode::BlockComment;
                index += 2;
            }
            Mode::Code if matches!(byte, b'\'' | b'"') => {
                quote = byte;
                triple = bytes.get(index + 1) == Some(&byte) && bytes.get(index + 2) == Some(&byte);
                raw_string = index > 0
                    && bytes[index - 1] == b'r'
                    && (index < 2 || !bytes[index - 2].is_ascii_alphanumeric());
                escaped = false;
                let delimiter = if triple { 3 } else { 1 };
                output.extend_from_slice(&bytes[index..index + delimiter]);
                index += delimiter;
                mode = Mode::String;
            }
            Mode::String => {
                if triple
                    && byte == quote
                    && bytes.get(index + 1) == Some(&quote)
                    && bytes.get(index + 2) == Some(&quote)
                {
                    output.extend_from_slice(&bytes[index..index + 3]);
                    index += 3;
                    mode = Mode::Code;
                } else {
                    output.push(byte);
                    if !triple && byte == quote && (raw_string || !escaped) {
                        mode = Mode::Code;
                    }
                    escaped = !raw_string && byte == b'\\' && !escaped;
                    if byte != b'\\' {
                        escaped = false;
                    }
                    index += 1;
                }
            }
            Mode::LineComment => {
                output.push(byte);
                if byte == b'\n' {
                    mode = Mode::Code;
                }
                index += 1;
            }
            Mode::BlockComment => {
                output.push(byte);
                if byte == b'*' && next == Some(b'/') {
                    output.push(b'/');
                    mode = Mode::Code;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            Mode::Code => {
                output.push(byte);
                index += 1;
            }
        }
    }
    output
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
