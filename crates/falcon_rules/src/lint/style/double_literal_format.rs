//! Flags double literals written with redundant or missing digits.
//!
//! Consistent numeric literals are easier to scan and diff. Three formatting
//! mistakes are reported: a redundant leading zero (`05.0`), a missing leading
//! zero before the decimal point (`.5`, which should be `0.5`), and a redundant
//! trailing zero in the fractional part (`1.50`). A literal whose fractional
//! part is exactly `0` (`24.0`) is deliberately left alone, since stripping that
//! digit would turn the `double` into an `int`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct DoubleLiteralFormat;

impl Rule for DoubleLiteralFormat {
    fn name(&self) -> &'static str {
        "double-literal-format"
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

/// Redundant leading zero: `05.0` / `00.5`. dcl: `startsWith('0') && lexeme[1] != '.'`.
fn detect_leading_zero(lexeme: &str) -> bool {
    lexeme.starts_with('0') && lexeme.as_bytes().get(1) != Some(&b'.')
}

/// Redundant trailing zero in the mantissa: `1.50` → `1.5`, `0.50` → `0.5`,
/// `1.00` → `1.0`. dcl deliberately does NOT flag `1.0` / `24.0` (fractional
/// part is exactly `"0"`), because stripping that zero would turn a `double`
/// into an `int`. This is the crux of the port fix: no int-ification.
fn detect_trailing_zero(lexeme: &str) -> bool {
    let mantissa = lexeme.split('e').next().unwrap_or(lexeme);
    mantissa.contains('.') && mantissa.ends_with('0') && mantissa.rsplit('.').next() != Some("0")
}

fn check_double(value: &str, span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Mirrors dart_code_linter's three formatting checks, in priority order.
    // Only literal *formatting* is checked; `24.0` → `24` (int-ification) is
    // explicitly out of scope.
    let message = if detect_leading_zero(value) {
        "Double literal shouldn't have redundant leading '0'."
    } else if value.starts_with('.') {
        "Double literal shouldn't begin with '.'."
    } else if detect_trailing_zero(value) {
        "Double literal shouldn't have a trailing '0'."
    } else {
        return;
    };
    diags.push(Diagnostic::new(
        "double-literal-format",
        Severity::Warning,
        message,
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Every double literal in the file, wherever it appears. The shared walker is
/// exhaustive over the AST, so new syntax cannot silently hide a literal.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::DoubleLit { value, span } = node {
            check_double(value, span, &mut self.diags, self.ctx);
        }
        walk_expr(self, node);
    }
}
