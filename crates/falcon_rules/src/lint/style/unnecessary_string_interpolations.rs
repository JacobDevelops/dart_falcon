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
//! text, or two adjacent interpolations like `'$a$b'`, is left alone. Raw strings
//! are never flagged. Use the interpolated expression on its own.
//!
//! Canonical receiver and signature facts must prove a non-nullable `String`.
//! Nullable or unknown values and null-aware accesses are left alone.

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

pub struct UnnecessaryStringInterpolations;
impl Rule for UnnecessaryStringInterpolations {
    fn name(&self) -> &'static str {
        "unnecessary-string-interpolations"
    }
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}
impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::StringLit(node) = node else {
            return;
        };
        let Some(lit) = StringLitNode::delimiter(&node.raw) else {
            return;
        };
        if !lit.terminated
            || lit.is_raw
            || !is_whole_interpolation(lit.content)
            || node.interpolations.len() != 1
        {
            return;
        }
        let ty = state.infer(&node.interpolations[0].expr);
        if ty.interface("dart:core", "String") && !ty.nullable() {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-string-interpolations",
                "Unnecessary string interpolation; use the expression directly.",
                node.span,
            ));
        }
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

// `$` is excluded so that `'$a$b'` (two interpolations) is not mistaken for one identifier.
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// True when `content` is exactly one interpolation and nothing else: `${...}` spanning the
/// whole content, or `$identifier` spanning the whole content.
fn is_whole_interpolation(content: &str) -> bool {
    let bytes = content.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'$' {
        return false;
    }
    if bytes[1] == b'{' {
        // `${...}` must close exactly at the final byte, with a non-empty expression.
        let mut depth = 0usize;
        let mut i = 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 1,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i == bytes.len() - 1 && i > 2;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    } else {
        // `$identifier` covering the whole content.
        is_ident_start(bytes[1]) && bytes[1..].iter().all(|&c| is_ident_continue(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SemanticModel, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    fn resolving(source: &str, check: impl FnOnce(&Program, &AnalyzeContext<'_>)) {
        let program = parse(source).0;
        let path = Path::new("test.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(path, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        check(&program, &ctx);
    }

    #[test]
    fn canonical_inference_preserves_numeric_and_string_joins() {
        struct Probe(Vec<falcon_analyze::ResolvedType>);
        impl SemanticRuleVisitor for Probe {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                if let Expr::Call { callee, args, .. } = node
                    && matches!(callee.as_ref(), Expr::Ident(id) if id.name == "probe")
                {
                    self.0
                        .extend(args.positional.iter().map(|expr| state.infer(expr)));
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
                assert_eq!(probe.0[14], falcon_analyze::ResolvedType::Unknown);
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
                    UnnecessaryStringInterpolations.analyze(program, ctx).len(),
                    3
                )
            },
        );
    }
}
