//! Flags `/** ... */` block documentation comments in favor of the `///` form.
//!
//! Dart supports two doc-comment syntaxes, but the community style guide standardizes
//! on the end-of-line `///` form; the `/** ... */` block form is discouraged. Using a
//! single style keeps doc comments consistent and easy to scan. Detection scans the
//! raw source with a small state machine so `/**` sequences inside strings, line
//! comments, or ordinary block comments are ignored, and the empty `/**/` comment is
//! not mistaken for a doc comment.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct SlashForDocComments;

impl Rule for SlashForDocComments {
    fn name(&self) -> &'static str {
        "slash-for-doc-comments"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for start in doc_block_comment_starts(ctx.source) {
            diags.push(Diagnostic::new(
                "slash-for-doc-comments",
                Severity::Warning,
                "Use the end-of-line form ('///') for doc comments.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start,
                    end: start + 3,
                },
            ));
        }
        diags
    }
}

/// Byte offsets of every `/**` doc-comment opener in `source`, skipping
/// occurrences inside strings or other comments and the empty `/**/` comment.
fn doc_block_comment_starts(source: &str) -> Vec<usize> {
    #[derive(PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Str(char),
    }

    let bytes = source.as_bytes();
    let mut state = State::Code;
    let mut starts = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match state {
            State::Code => match b {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    state = State::Line;
                    i += 2;
                    continue;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    // Doc comment when a third `*` follows but not `/**/`.
                    if bytes.get(i + 2) == Some(&b'*') && bytes.get(i + 3) != Some(&b'/') {
                        starts.push(i);
                    }
                    state = State::Block;
                    i += 2;
                    continue;
                }
                b'\'' | b'"' => {
                    state = State::Str(b as char);
                }
                _ => {}
            },
            State::Line => {
                if b == b'\n' {
                    state = State::Code;
                }
            }
            State::Block => {
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    state = State::Code;
                    i += 2;
                    continue;
                }
            }
            State::Str(q) => match b {
                b'\\' => {
                    i += 2;
                    continue;
                }
                _ if b as char == q => {
                    state = State::Code;
                }
                _ => {}
            },
        }
        i += 1;
    }
    starts
}
