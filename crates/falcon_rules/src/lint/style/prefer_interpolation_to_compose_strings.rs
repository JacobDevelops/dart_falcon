//! Prefers string interpolation to composing strings with `+`.
//!
//! Each `+` chain (`a + b + c`, flattened through unparenthesized left
//! operands) is scanned pair by pair: a pair is reported, from its left
//! operand to its right one, when exactly one side is a string literal (plain,
//! interpolated, or adjacent), neither side is a single raw literal (`r'...'`)
//! nor a `toString(...)` call with arguments, and the left operand is a
//! `String`. The pair after a reported one is skipped. A non-`+` binary
//! expression as a chain's left operand ends the chain (`a - b + 'x'` checks
//! only `'x'`), and a parenthesized operand is an ordinary operand.
//!
//! falcon stays silent when it cannot infer the left operand's type.
//!
//! Adopted from the official `prefer_interpolation_to_compose_strings` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferInterpolationToComposeStrings;

impl Rule for PreferInterpolationToComposeStrings {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            reported: HashSet::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Reported ranges: every `+` in a chain rescans its own prefix.
    reported: HashSet<(usize, usize)>,
}

/// One operand of a `+` chain.
#[derive(Clone, Copy)]
struct Operand<'a> {
    expr: Expr<'a>,
}

impl<'a> Operand<'a> {
    fn string_literal(self) -> Option<StringLitNode<'a>> {
        match self.expr {
            Expr::StringLit(string) => Some(string),
            _ => None,
        }
    }

    /// `x.toString(...)` or `toString(...)` with arguments.
    fn to_string_with_arguments(self) -> bool {
        matches!(self.expr, Expr::Call(call)
        if !(call.args().positional().is_empty() && call.args().named().is_empty())
            && match call.callee() {
                Expr::Field(field) => field.field().name() == "toString",
                Expr::Ident(name) => name.name() == "toString",
                _ => false,
            })
    }
}

/// The operands of the `+` chain `node` (flattened through unparenthesized
/// left operands); a non-`+` binary left operand contributes none.
fn chained_additions<'a>(node: Expr<'a>) -> Vec<Operand<'a>> {
    let mut operands = Vec::new();
    collect_chain(node, &mut operands);
    operands
}

fn collect_chain<'a>(node: Expr<'a>, operands: &mut Vec<Operand<'a>>) {
    let Expr::Binary(binary) = node else {
        operands.push(Operand { expr: node });
        return;
    };
    if binary.op() != BinaryOp::Add {
        return;
    }
    collect_chain(binary.left(), operands);
    operands.push(Operand {
        expr: binary.right(),
    });
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(node, Expr::Binary(binary) if binary.op() == BinaryOp::Add) {
            return;
        }
        let operands = chained_additions(node);
        let mut index = 0;
        while index + 1 < operands.len() {
            let (left, right) = (operands[index], operands[index + 1]);
            index += 1;
            let (left_literal, right_literal) = (left.string_literal(), right.string_literal());
            let raw = |literal: Option<StringLitNode<'a>>| {
                literal.is_some_and(|string| string.has_raw_segment() && !string.is_adjacent())
            };
            if left_literal.is_some() == right_literal.is_some()
                || raw(left_literal)
                || raw(right_literal)
                || left.to_string_with_arguments()
                || right.to_string_with_arguments()
                || !state.infer(left.expr).interface("dart:core", "String")
            {
                continue;
            }
            let span = Span::new(left.expr.span().start, right.expr.span().end);
            if self.reported.insert((span.start, span.end)) {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-interpolation-to-compose-strings",
                    "Use string interpolation to compose strings and values.",
                    span,
                ));
            }
            // The right operand was just reported; skip over it.
            index += 1;
        }
    }
}
