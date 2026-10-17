//! Flags multi-line argument lists that are missing a trailing comma.
//!
//! A trailing comma is required only when the argument list is already broken
//! across lines — when the last significant token sits on an earlier line than
//! the closing `)`. This mirrors the Dart 3.x tall-style formatter.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct PreferTrailingComma;

impl Rule for PreferTrailingComma {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> Collector<'a> {
    fn check_args(&mut self, args: ArgList<'a>) {
        if args.positional().is_empty() && args.named().is_empty() {
            return;
        }
        let mut tokens = self
            .ctx
            .tree()
            .tokens_in(self.ctx.source, args.span())
            .rev()
            .filter(|token| !token.is_trivia());
        let Some(close) = tokens
            .next()
            .filter(|token| token.kind() == TokenKind::RParen)
        else {
            return;
        };
        let Some(last) = tokens.next() else { return };
        if last.kind() == TokenKind::Comma
            || !self.ctx.source[last.offset() + last.len()..close.offset()].contains('\n')
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "prefer-trailing-comma",
            "Add a trailing comma to the argument list",
            DiagSpan {
                start: close.offset(),
                end: close.offset() + close.len(),
            },
        ));
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_annotation(&mut self, node: Annotation<'a>) -> StructuralTraversal {
        if let Some(args) = node.args() {
            self.check_args(args);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        for initializer in node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall(call) => self.check_args(call.args()),
                ConstructorInitializer::ThisCall(call) => self.check_args(call.args()),
                ConstructorInitializer::FieldInit(_)
                | ConstructorInitializer::Assert(_) => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) -> StructuralTraversal {
        for variant in node.variants() {
            if let Some(args) = variant.args() {
                self.check_args(args);
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::Call(call) => self.check_args(call.args()),
            Expr::New(construction) => self.check_args(construction.args()),
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    for op in section.ops() {
                        match op {
                            CascadeOp::Call(call) => self.check_args(call.args()),
                            CascadeOp::Invoke(call) => self.check_args(call.args()),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn retained_argument_endpoints_preserve_exact_closing_spans() {
        for (source, expected) in [
            ("final x = call(\n  nested(')'),\n  2 /* ) */\n);", true),
            ("final x = call(\n  1, // )\n);", false),
            ("final x = call(\n  '''first\nlast''');", false),
            ("final x = call(\n  'first' 'last'\n);", true),
            ("final x = call(/* ) */\n);", false),
            ("final x = '${call(\n  1\n)}';", true),
            ("final x = call(\n  r'\\'\n);", true),
            ("final x = call(\n  '''it's'''\n);", true),
            ("final x = call(\n  \"${f(\")\")}\"\n);", true),
        ] {
            let parsed = falcon_dart_parser::parse(source);
            assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
            let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: parsed.tree,
            });
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
            let diagnostics = PreferTrailingComma.analyze(parsed.program(), &ctx);
            let spans: Vec<_> = diagnostics.iter().map(|diag| diag.span).collect();
            let expected: Vec<_> = if expected {
                let close = source.rfind(')').unwrap();
                vec![DiagSpan {
                    start: close,
                    end: close + 1,
                }]
            } else {
                Vec::new()
            };
            assert_eq!(spans, expected, "{source}");
        }
    }
}
