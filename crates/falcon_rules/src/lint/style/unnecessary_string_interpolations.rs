//! Flags a string literal whose entire content is a single interpolation of a
//! provably non-nullable `String`. Ported from package:lints
//! `unnecessary_string_interpolations`.
//!
//! When a string is nothing but `'$x'` or `'${x}'` and `x` is already a string,
//! wrapping the value in quotes and interpolation markers is redundant — the
//! literal is equivalent to writing the expression directly. The quotes only add
//! noise and obscure that no formatting or concatenation is happening. The rule
//! requires the interpolation to span the whole content, whether a bare
//! `$identifier` or a full `${...}` expression; a string with any surrounding
//! text, or two adjacent interpolations like `'$a$b'`, is left alone. Adjacent
//! literal segments and raw literals are never flagged.
//! Use the interpolated expression on its own.
//!
//! Canonical receiver and signature facts must prove a non-nullable `String`.
//! Nullable or unknown values and null-aware accesses are left alone.

use crate::lint::string_lit::{interpolated_literals, multiline_lead_len};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryStringInterpolations;
impl Rule for UnnecessaryStringInterpolations {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::StringLit(node) = node else {
            return;
        };
        if node.is_adjacent() {
            return;
        }
        let mut interpolations = node.interpolations();
        let (Some(interpolation), None) = (interpolations.next(), interpolations.next()) else {
            return;
        };
        for (lit, span) in interpolated_literals(node) {
            let trim = if lit.triple {
                multiline_lead_len(lit.content)
            } else {
                0
            };
            let content = &lit.content[trim..];
            let start = span.start + lit.content_offset + trim;
            let (prefix, suffix) = if content.starts_with("${") && content.ends_with('}') {
                (2, 1)
            } else if content.starts_with('$') {
                (1, 0)
            } else {
                continue;
            };
            if interpolation.span().start != start + prefix
                || interpolation.span().end != start + content.len() - suffix
            {
                continue;
            }
            let ty = state.infer(interpolation.expr());
            if ty.interface("dart:core", "String") && !ty.nullable() {
                self.diags.push(self.ctx.diagnostic("Use the expression directly instead of this unnecessary string interpolation.", span));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_analyze::visit_program;

    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    fn resolving(source: &str, check: impl for<'a> FnOnce(Program<'a>, &'a AnalyzeContext<'a>)) {
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parse(source).tree,
        });
        let program = parsed.program();
        let path = Path::new("test.dart");
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, true);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        check(program, &ctx);
    }

    #[test]
    fn canonical_inference_preserves_numeric_and_string_joins() {
        struct Probe(Vec<falcon_analyze::ResolvedType>);
        impl<'a> SemanticRuleVisitor<'a> for Probe {
            fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
                if let Expr::Call(call) = node
                    && matches!(call.callee().unparenthesized(), Expr::Ident(id) if id.name() == "probe")
                {
                    self.0
                        .extend(call.args().positional().map(|expr| state.infer(expr)));
                }
            }
        }
        resolving(
            "String text() => 'x'; void f(int x, num n, String b, String? maybe) {
                probe(x + 1.0, x + 1, n + x, 'a' + b, text().toUpperCase(), maybe, maybe!);
                var value = true; probe(value);
                { var value = 1; probe(value); } probe(value);
                probe(x as double, [1], {'a': 1}, x > 0, unknown);
            }",
            |program, ctx| {
                let mut probe = Probe(Vec::new());
                visit_program(
                    &mut probe,
                    program,
                    SemanticState::new(program, ctx).unwrap(),
                );
                for (ty, name) in probe.0.iter().zip([
                    "double", "int", "num", "String", "String", "String", "String", "bool", "int",
                    "bool", "double", "List", "Map", "bool",
                ]) {
                    assert!(ty.interface("dart:core", name), "{ty:?} != {name}");
                }
                assert_eq!(probe.0.len(), 15);
                assert!(probe.0[5].nullable());
                assert!(!probe.0[6].nullable());
                assert_eq!(probe.0[14], falcon_analyze::ResolvedType::UNKNOWN);
            },
        );
    }

    #[test]
    fn interpolation_requires_nonnullable_canonical_string() {
        resolving(
            "void f(String text, String? maybe) {
                print('$text'); print('${text.toUpperCase()}'); print('${'a' + text}');
                print('$maybe'); print('${maybe?.toUpperCase()}'); print('$unknown');
            }",
            |program, ctx| {
                assert_eq!(
                    UnnecessaryStringInterpolations.analyze(UnnecessaryStringInterpolations::NAME, program, ctx).len(),
                    3
                )
            },
        );
    }

    #[test]
    fn interpolation_boundaries_use_parsed_expression_spans() {
        resolving(
            r#"void f(String text) {
                print('${"}"}');
                print('${text /* } */}');
                print(''' 
$text''');
                print('x$text'); print('$text '); print('$text$text');
                print(r'$text'); print('$text' '');
            }"#,
            |program, ctx| {
                assert_eq!(
                    UnnecessaryStringInterpolations.analyze(UnnecessaryStringInterpolations::NAME, program, ctx).len(),
                    3
                );
            },
        );
    }
}
