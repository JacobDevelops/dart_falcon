//! Flags an argument passed more than once in the same invocation.
//!
//! When two arguments to a call or constructor have identical source text — such
//! as `Point(x, x)` where one was meant to be `y` — the repetition is usually a
//! copy-paste slip that silently produces wrong results. Positional arguments are
//! compared against other positional arguments by source text, and named
//! arguments against other named arguments by their value expression (the label
//! is ignored); a positional is never matched against a named. Literal-valued
//! arguments are excluded, since repeating a literal like `Size(48, 48)` is
//! intentional. The report lands on the last duplicate.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct NoEqualArguments;

impl Rule for NoEqualArguments {
    fn name(&self) -> &'static str {
        "no-equal-arguments"
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

/// A literal argument, per dcl semantics, is compared by identity only, so two
/// distinct literals never count as "equal". This mirrors dart_code_linter's
/// `_bothLiterals` short-circuit (`argument == arg`): passing `Size(48, 48)` or
/// `copyWith(isSaving: false, isSaved: false)` is intentional, never a bug.
/// Matches the analyzer's `Literal` hierarchy: scalar literals, collection
/// literals (`TypedLiteral`), and a prefix expression whose operand is a literal
/// (e.g. `-1`).
fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::List { .. }
        | Expr::Map { .. }
        | Expr::Set { .. } => true,
        Expr::Unary { operand, .. } => is_literal(operand),
        _ => false,
    }
}

/// Report the *last* occurrence of each duplicated argument, matching dcl's
/// `lastAppearance` behaviour. Reporting on the last (not first) occurrence is
/// what lets hand-written `// falcon-ignore lint/suspicious/no-equal-arguments`
/// comments — which developers place on the trailing duplicate — line up and
/// suppress the hit.
fn check_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Positional args match other positional args by full source text; named
    // args match other named args by their *value* expression text (the label
    // is ignored). dcl never matches a positional against a named argument.
    // Literal-valued arguments are excluded entirely (compared by identity).
    let positional: Vec<(&str, &Span)> = args
        .positional
        .iter()
        .filter(|a| !is_literal(a))
        .map(|a| (expr_src(a, ctx.source), a.span()))
        .collect();
    let named: Vec<(&str, &Span)> = args
        .named
        .iter()
        .filter(|n| !is_literal(&n.value))
        .map(|n| (expr_src(&n.value, ctx.source), &n.span))
        .collect();

    for group in [&positional, &named] {
        report_duplicates(group, diags, ctx);
    }
}

fn report_duplicates(entries: &[(&str, &Span)], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for (i, (src, _)) in entries.iter().enumerate() {
        // Index of the last entry equal to this one.
        let last = entries
            .iter()
            .rposition(|(other, _)| other == src)
            .unwrap_or(i);
        // Only the *earlier* duplicates trigger a report, and the report lands
        // on the last occurrence. Emit once per group by acting on the first
        // member only.
        if last != i && entries[..i].iter().all(|(other, _)| other != src) {
            let span = entries[last].1;
            diags.push(Diagnostic::new(
                "no-equal-arguments",
                Severity::Warning,
                "The argument has already been passed",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

fn expr_src<'a>(expr: &Expr, source: &'a str) -> &'a str {
    let span = expr.span();
    let end = span.end.min(source.len());
    &source[span.start..end]
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
        match node {
            Expr::Call { args, .. } | Expr::New { args, .. } => {
                check_args(args, &mut self.diags, self.ctx);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
