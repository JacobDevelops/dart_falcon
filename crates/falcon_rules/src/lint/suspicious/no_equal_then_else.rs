//! Flags an `if`/`else` or ternary whose two branches are identical.
//!
//! When the then and else branches have the same body, the condition decides
//! nothing — the same code runs either way — so either the condition is pointless
//! or one branch was never edited to differ. Comparison is on branch source text
//! with whitespace outside strings and comments normalized away, so formatting
//! differences do not hide a match while literal and comment contents stay intact. Remove the condition and keep the single body, or correct
//! the branch that was supposed to differ. Applies to both `if`/`else` statements
//! and `?:` ternary expressions.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoEqualThenElse;

impl Rule for NoEqualThenElse {
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

pub(crate) fn normalize<'a>(
    ctx: &'a AnalyzeContext<'_>,
    span: Span,
) -> impl Iterator<Item = u8> + 'a {
    use falcon_syntax::TokenKind;

    let mut line_comment = false;
    ctx.tree()
        .tokens_in(ctx.source, span)
        .flat_map(move |token| {
            let mut text = token.text();
            let in_literal = token.parent().ancestors().any(|node| {
                node.kind() == NodeKind::StringLitNode
                    && node.span().start >= span.start
                    && node.span().end <= span.end
            });
            let preserve_whitespace = if in_literal {
                true
            } else if line_comment {
                // The retained comment excludes its terminating newline; equality
                // includes that newline and any preceding CR.
                line_comment = token.kind() != TokenKind::Newline;
                true
            } else {
                if token.kind().is_comment() && text.starts_with("/* expect:") {
                    text = "";
                } else if matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment)
                    && text.starts_with("//")
                {
                    line_comment = true;
                }
                token.kind().is_comment()
                    || matches!(token.kind(), TokenKind::StringLit | TokenKind::StringPart)
            };
            text.bytes()
                .filter(move |byte| preserve_whitespace || !byte.is_ascii_whitespace())
        })
}

fn last_stmt_span(stmt: Stmt<'_>) -> Span {
    match stmt {
        Stmt::Block(b) => b.stmts().last().map(last_stmt_span).unwrap_or(b.span()),
        other => other.span(),
    }
}

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
        if let Expr::Conditional(conditional) = node
            && normalize(self.ctx, conditional.then_expr().span()).eq(normalize(self.ctx, conditional.else_expr().span()))
        {
            self.diags.push(self.ctx.diagnostic(
                "no-equal-then-else",
                "Both branches of ternary are identical — remove the condition",
                conditional.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(i) = node
            && let Some(else_branch) = i.else_branch()
            && normalize(self.ctx, i.then_branch().span())
                .eq(normalize(self.ctx, else_branch.span()))
        {
            let last = last_stmt_span(i.then_branch());
            self.diags.push(self.ctx.diagnostic(
                "no-equal-then-else",
                "Both branches of if/else are identical — remove the condition",
                last,
            ));
        }
        StructuralTraversal::Continue
    }
}
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use falcon_syntax::cst::Span;
    use std::path::Path;

    fn normalized(source: &str) -> Vec<u8> {
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parse(source).tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        normalize(&ctx, Span::new(0, source.len())).collect()
    }

    #[test]
    fn branch_equality_preserves_comment_contents() {
        assert_eq!(normalized("run( 1 );"), normalized("run(1);"));
        assert_ne!(
            normalized("run(); /* a b */"),
            normalized("run(); /* ab */")
        );
        assert_ne!(normalized("run(); // a b\n"), normalized("run(); // ab\n"));
        assert_ne!(normalized("run(); // same\n"), normalized("run(); // same"));
        assert_ne!(
            normalized("run(); // same\r\n"),
            normalized("run(); // same\n")
        );
        assert_eq!(normalized("// same\r'''\n A '''"), b"// same\r'''\n A '''");
        assert_eq!(
            normalized("run(); /* expect: no-equal-then-else */"),
            normalized("run();")
        );
        assert_ne!(
            normalized("run(); /* expect : kept */"),
            normalized("run();")
        );
        assert_ne!(
            normalized("run(); /* outer /* inner */ a b */"),
            normalized("run(); /* outer /* inner */ ab */")
        );
    }

    #[test]
    fn branch_equality_preserves_complete_literals() {
        assert_ne!(normalized("'a b'"), normalized("'ab'"));
        assert_ne!(normalized("r'a b'"), normalized("r'ab'"));
        assert_ne!(normalized("'''a b'''"), normalized("'''ab'''"));
        assert_ne!(normalized("'/* expect: kept */'"), normalized("''"));
        assert_ne!(normalized("'${a + b}'"), normalized("'${a+b}'"));
        assert_ne!(normalized("'${'a b'}'"), normalized("'${'ab'}'"));
    }
}
