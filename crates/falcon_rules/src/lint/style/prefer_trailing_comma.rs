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
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct PreferTrailingComma;

impl Rule for PreferTrailingComma {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Collector<'_, '_> {
    fn check_args(&mut self, args: &ArgList) {
        if args.positional.is_empty() && args.named.is_empty() {
            return;
        }
        let mut tokens = self
            .ctx
            .syntax
            .tokens_in(args.span)
            .rev()
            .filter(|token| !token.is_trivia());
        let Some(close) = tokens
            .next()
            .filter(|token| token.kind == TokenKind::RParen)
        else {
            return;
        };
        let Some(last) = tokens.next() else { return };
        if last.kind == TokenKind::Comma
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
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_annotation(&mut self, node: &Annotation) -> StructuralTraversal {
        if let Some(args) = &node.args {
            self.check_args(args);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        for initializer in node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => self.check_args(args),
                ConstructorInitializer::FieldInit { .. }
                | ConstructorInitializer::Assert { .. } => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) -> StructuralTraversal {
        for variant in &node.variants {
            if let Some(args) = &variant.args {
                self.check_args(args);
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        match node {
            Expr::Call { args, .. } | Expr::New { args, .. } => self.check_args(args),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    for op in &section.ops {
                        if let CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args) = op {
                            self.check_args(args);
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
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
            let diagnostics = PreferTrailingComma.analyze(&parsed.program, &ctx);
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
