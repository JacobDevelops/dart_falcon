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
//! Each left spine is collected iteratively once; preorder visits its prefixes
//! before any operand changes semantic flow, so the root already covers every
//! greedy pair a prefix would report.
//!
//! falcon stays silent when it cannot infer the left operand's type.
//!
//! Adopted from the official `prefer_interpolation_to_compose_strings` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

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
            visited: HashSet::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Addition prefixes already scanned by their outermost left-spine root.
    visited: HashSet<NodeId>,
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
fn collect_chain<'a>(mut node: Expr<'a>, visited: &mut HashSet<NodeId>) -> Vec<Operand<'a>> {
    let mut operands = Vec::new();
    loop {
        let Expr::Binary(binary) = node else {
            operands.push(Operand { expr: node });
            break;
        };
        if binary.op() != BinaryOp::Add {
            break;
        }
        visited.insert(binary.syntax().id());
        operands.push(Operand {
            expr: binary.right(),
        });
        node = binary.left();
    }
    operands.reverse();
    operands
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if binary.op() != BinaryOp::Add || self.visited.contains(&binary.syntax().id()) {
            return;
        }
        let operands = collect_chain(node, &mut self.visited);
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
            self.diags.push(self.ctx.diagnostic("Use string interpolation to compose strings and values.", span));
            // The right operand was just reported; skip over it.
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        crate::lint::test_support::diagnostics(
            PreferInterpolationToComposeStrings::NAME,
            &PreferInterpolationToComposeStrings,
            Path::new("/project/lib/test.dart"),
            source,
            false,
        )
    }

    #[test]
    fn numeric_chain_collects_each_prefix_once_on_normal_stack() {
        let source = format!("void f(int a) {{ {}; }}", vec!["a"; 100_000].join(" + "));
        assert!(diagnostics(&source).is_empty());
    }

    #[test]
    fn mixed_chains_preserve_greedy_pairs_filters_and_diagnostic_order() {
        let source = r#"
void f(String a, String b, String c) {
  'a' + a + 'b' + b + 'c' + c;
  a + b + 'd' + c + 'e';
  r'raw' + a + 'f' + b.toString(1) + 'g' + c;
  ('h' + a) + b + 'i';
  'a' - a + 'j' + c;
  'hello $a' + b + 'adj' 'acent' + c;
  r'raw' 'adjacent' + a;
  'x' + a.toString();
}
"#;
        let actual: Vec<_> = diagnostics(source)
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let expected: Vec<_> = [
            "'a' + a",
            "'b' + b",
            "'c' + c",
            "b + 'd'",
            "c + 'e'",
            "a + 'f'",
            "'g' + c",
            "b + 'i'",
            "'h' + a",
            "'j' + c",
            "'hello $a' + b",
            "'adj' 'acent' + c",
            "r'raw' 'adjacent' + a",
            "'x' + a.toString()",
        ]
        .into_iter()
        .map(|pair| {
            let start = source.find(pair).unwrap();
            (start, start + pair.len())
        })
        .collect();
        assert_eq!(actual, expected);
    }
}
