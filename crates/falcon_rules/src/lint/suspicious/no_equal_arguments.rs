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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoEqualArguments;

impl Rule for NoEqualArguments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

/// A literal argument, per dcl semantics, is compared by identity only, so two
/// distinct literals never count as "equal". This mirrors dart_code_linter's
/// `_bothLiterals` short-circuit (`argument == arg`): passing `Size(48, 48)` or
/// `copyWith(isSaving: false, isSaved: false)` is intentional, never a bug.
/// Matches the analyzer's `Literal` hierarchy: scalar literals, collection
/// literals (`TypedLiteral`), and a prefix expression whose operand is a literal
/// (e.g. `-1`).
fn is_literal(expr: Expr<'_>) -> bool {
    match expr.unparenthesized() {
        Expr::IntLit(_)
        | Expr::DoubleLit(_)
        | Expr::StringLit(_)
        | Expr::BoolLit(_)
        | Expr::NullLit(_)
        | Expr::List(_)
        | Expr::Map(_)
        | Expr::Set(_) => true,
        Expr::Unary(unary) => is_literal(unary.operand()),
        _ => false,
    }
}

/// Report the *last* occurrence of each duplicated argument, matching dcl's
/// `lastAppearance` behaviour. Reporting on the last (not first) occurrence is
/// what lets hand-written `// falcon-ignore lint/suspicious/no-equal-arguments`
/// comments — which developers place on the trailing duplicate — line up and
/// suppress the hit.
fn check_args(args: ArgList<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Positional args match other positional args by full source text; named
    // args match other named args by their *value* expression text (the label
    // is ignored). dcl never matches a positional against a named argument.
    // Literal-valued arguments are excluded entirely (compared by identity).
    let positional: Vec<(&str, Span)> = args
        .positional()
        .filter(|a| !is_literal(*a))
        .map(|a| (expr_src(a, ctx.source()), a.span()))
        .collect();
    let named: Vec<(&str, Span)> = args
        .named()
        .filter(|n| !is_literal(n.value()))
        .map(|n| (expr_src(n.value(), ctx.source()), n.span()))
        .collect();

    for group in [&positional, &named] {
        report_duplicates(group, diags, ctx);
    }
}

fn report_duplicates(entries: &[(&str, Span)], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
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
            diags.push(ctx.diagnostic("The argument has already been passed.", span));
        }
    }
}

use crate::lint::expressions::expr_src;

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::Call(call) => check_args(call.args(), &mut self.diags, self.ctx),
            Expr::New(new) => check_args(new.args(), &mut self.diags, self.ctx),
            // A cascade call carries its own ArgList and is never rebuilt as an
            // Expr::Call, so the arm above never sees `a..f(x, x)`.
            Expr::Cascade(cascade) => {
                for op in cascade.sections().flat_map(|s| s.ops()) {
                    match op {
                        CascadeOp::Call(call) => check_args(call.args(), &mut self.diags, self.ctx),
                        CascadeOp::Invoke(invoke) => check_args(invoke.args(), &mut self.diags, self.ctx),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}
