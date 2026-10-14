//! Flags a `library` directive that has neither a doc comment nor annotations.
//!
//! A library directive is only needed to anchor library-level documentation or
//! annotations (and, historically, a name). Without either it is noise:
//!
//! ```dart
//! library;                 // flagged
//!
//! /// Utilities for parsing.
//! library;                 // fine: documented
//!
//! @TestOn('browser')
//! library;                 // fine: annotated
//! ```
//!
//! Libraries that declare `part` files are skipped, since parts may still refer
//! to the library by name.
//!
//! Adopted from the official `unnecessary_library_directive` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct UnnecessaryLibraryDirective;

impl Rule for UnnecessaryLibraryDirective {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(lib) = &program.library_directive else {
            return Vec::new();
        };
        if !program.part_directives.is_empty() || !lib.annotations.is_empty() {
            return Vec::new();
        }
        if has_doc_comment(ctx.source, lib.span.start) {
            return Vec::new();
        }
        vec![ctx.diagnostic(
            "unnecessary-library-directive",
            "Library directives without comments or annotations should be avoided.",
            lib.span,
        )]
    }
}

/// Whether the comments directly preceding `offset` include a doc comment.
fn has_doc_comment(source: &str, offset: usize) -> bool {
    let tokens = Lexer::new(source).tokenize();
    let before = tokens.partition_point(|token| token.offset < offset);
    tokens[..before]
        .iter()
        .rev()
        .take_while(|token| token.is_trivia())
        .any(|token| token.kind == TokenKind::DocComment)
}
