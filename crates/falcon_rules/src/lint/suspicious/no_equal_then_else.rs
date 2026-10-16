//! Flags an `if`/`else` or ternary whose two branches are identical.
//!
//! When the then and else branches have the same body, the condition decides
//! nothing — the same code runs either way — so either the condition is pointless
//! or one branch was never edited to differ. Comparison is on branch source text
//! with whitespace outside strings and comments normalized away, so formatting
//! differences do not hide a match while literal and comment contents stay intact. Remove the condition and keep the single body, or correct
//! the branch that was supposed to differ. Applies to both `if`/`else` statements
//! and `?:` ternary expressions.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct NoEqualThenElse;

impl Rule for NoEqualThenElse {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

fn normalize<'a>(ctx: &'a AnalyzeContext<'_>, span: Span) -> impl Iterator<Item = u8> + 'a {
    use falcon_syntax::TokenKind;

    let mut line_comment = false;
    ctx.syntax.tokens_in(span).flat_map(move |token| {
        let mut text = token.text(ctx.source);
        let preserve_whitespace = if line_comment {
            // The retained comment excludes its terminating newline; equality
            // includes that newline and any preceding CR.
            line_comment = token.kind != TokenKind::Newline;
            true
        } else {
            if token.kind.is_comment() && text.starts_with("/* expect:") {
                text = "";
            } else if matches!(token.kind, TokenKind::LineComment | TokenKind::DocComment)
                && text.starts_with("//")
            {
                line_comment = true;
            }
            token.kind.is_comment() || token.kind == TokenKind::StringLit
        };
        text.bytes()
            .filter(move |byte| preserve_whitespace || !byte.is_ascii_whitespace())
    })
}

fn last_stmt_span(stmt: &Stmt) -> &Span {
    match stmt {
        Stmt::Block(b) => b.stmts.last().map(last_stmt_span).unwrap_or(&b.span),
        other => other.span(),
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Conditional {
            then_expr,
            else_expr,
            span,
            ..
        } = node
            && normalize(self.ctx, *then_expr.span()).eq(normalize(self.ctx, *else_expr.span()))
        {
            self.diags.push(self.ctx.diagnostic(
                "no-equal-then-else",
                "Both branches of ternary are identical — remove the condition",
                *span,
            ));
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(i) = node
            && let Some(else_branch) = &i.else_branch
            && normalize(self.ctx, *i.then_branch.span())
                .eq(normalize(self.ctx, *else_branch.span()))
        {
            let last = last_stmt_span(&i.then_branch);
            self.diags.push(self.ctx.diagnostic(
                "no-equal-then-else",
                "Both branches of if/else are identical — remove the condition",
                *last,
            ));
        }
        walk_stmt(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use falcon_syntax::ast::Span;
    use std::path::Path;

    fn normalized(source: &str) -> Vec<u8> {
        let parsed = falcon_dart_parser::parse(source);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
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
