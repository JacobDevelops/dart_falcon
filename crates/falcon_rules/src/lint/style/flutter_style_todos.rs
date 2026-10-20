//! Flags `TODO` comments that do not follow the Flutter style
//! `// TODO(username): message, https://URL-to-issue`.
//!
//! A TODO names who can explain it and, ideally, links an issue. A line
//! comment starting with `TODO` (any case, any number of slashes) must read
//! `// TODO(name): ` with a name made of letters, digits, `-`, and `.`:
//!
//! ```dart
//! // TODO: implement            flagged
//! // todo(user) missing colon   flagged
//! /// TODO(user): add docs      flagged: doc comments are not TODOs
//! // TODO(user): implement, https://github.com/org/repo/issues/1
//! ```
//!
//! Adopted from the official `flutter_style_todos` lint.

use std::sync::LazyLock;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::Program;
use falcon_syntax::token::TokenKind;
use regex::Regex;

static TODO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^//+\s*TODO\b").expect("valid TODO regex"));

static EXPECTED_TODO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^//\s*TODO\([a-zA-Z0-9][-a-zA-Z0-9.]*\): ").expect("valid Flutter TODO regex")
});

#[derive(Default)]
pub struct FlutterStyleTodos;

impl Rule for FlutterStyleTodos {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(
            ctx.tree()
                .comment_tokens(ctx.source())
                .filter(|token| {
                    matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment)
                })
                .filter(|token| {
                    let comment = token.text();
                    TODO.is_match(comment) && !EXPECTED_TODO.is_match(comment)
                })
                .map(|token| {
                    ctx.diagnostic(
                        "flutter-style-todos",
                        "To-do comment doesn't follow the Flutter style.",
                        DiagSpan {
                            start: token.offset(),
                            end: token.offset() + token.len(),
                        },
                    )
                })
                .collect(),
        )
    }
}
