//! Require explanations for analyzer `ignore:` and `ignore_for_file:` comments.
//!
//! Accept trailing prose (optionally after a second `//`) or an immediately
//! preceding standalone comment. Blank lines, trailing code comments, empty
//! comments, and other ignore directives do not supply an explanation.
//! Read CST comment tokens so directive-like text in strings is never linted.
//!
//! `ignore_global` (bool, default `true`) skips `ignore_for_file:` directives.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct PreferCommentingAnalyzerIgnores;

fn directive(text: &str) -> Option<&str> {
    let text = text.strip_prefix("//")?.trim_start();
    text.strip_prefix("ignore:")
        .or_else(|| text.strip_prefix("ignore_for_file:"))
}

fn inline_explanation(mut rest: &str) -> bool {
    loop {
        rest = rest.trim_start();
        let end = rest
            .find(|character: char| {
                !character.is_ascii_alphanumeric() && !matches!(character, '_' | '-' | '=')
            })
            .unwrap_or(rest.len());
        if end == 0 {
            return false;
        }
        rest = rest[end..].trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next;
            continue;
        }
        let reason = rest.strip_prefix("//").unwrap_or(rest).trim_start();
        return reason.chars().any(char::is_alphanumeric);
    }
}

fn explanation(text: &str) -> bool {
    let text = text.trim();
    if directive(text).is_some() {
        return false;
    }
    let body = text
        .strip_prefix("//")
        .or_else(|| {
            text.strip_prefix("/*")
                .and_then(|text| text.strip_suffix("*/"))
        })
        .unwrap_or("");
    body.chars().any(char::is_alphanumeric)
}

impl Rule for PreferCommentingAnalyzerIgnores {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diagnostics = Vec::new();
        let ignore_global =
            crate::lint::options::for_rule(ctx, "prefer-commenting-analyzer-ignores")
                .and_then(|options| options.get("ignore_global"))
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
        let mut previous_comment = None::<SyntaxToken<'a>>;
        let mut previous_line_comment = None::<SyntaxToken<'a>>;
        for token in ctx.tree().tokens(ctx.source) {
            if matches!(token.kind(), TokenKind::Whitespace | TokenKind::Newline) {
                continue;
            }
            if !token.kind().is_comment() {
                previous_comment = None;
                previous_line_comment = None;
                continue;
            }
            let current_line_start = ctx.source[..token.span().start]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            if previous_comment.is_some_and(|previous| previous.span().end <= current_line_start) {
                previous_line_comment = previous_comment;
            }
            if token.kind() == TokenKind::LineComment
                && let Some(rest) = directive(token.text())
                && !(ignore_global
                    && token
                        .text()
                        .trim_start_matches('/')
                        .trim_start()
                        .starts_with("ignore_for_file:"))
            {
                let inline_explanation = inline_explanation(rest);
                let preceding_explanation = previous_line_comment.is_some_and(|previous| {
                    let gap = &ctx.source[previous.span().end..current_line_start];
                    let line_start = ctx.source[..previous.span().start]
                        .rfind('\n')
                        .map_or(0, |index| index + 1);
                    gap.trim().is_empty()
                        && gap.bytes().filter(|byte| *byte == b'\n').count() == 1
                        && ctx.source[line_start..previous.span().start]
                            .trim()
                            .is_empty()
                        && explanation(previous.text())
                });
                let has_code =
                    rest.trim_start().chars().next().is_some_and(|character| {
                        character.is_ascii_alphanumeric() || character == '_'
                    });
                if has_code && !inline_explanation && !preceding_explanation {
                    diagnostics.push(ctx.diagnostic(
                        "prefer-commenting-analyzer-ignores",
                        "This analyzer ignore has no explanation. Add trailing explanatory text or an immediately preceding explanation comment.",
                        token.span(),
                    ));
                }
            }
            previous_comment = Some(token);
        }
        RulePreparation::Diagnostics(diagnostics)
    }
}
