//! Regression tests for lexer gaps in the `lexer` implementation group:
//! doc comments (`/** */`), symbol-literal `#`, escapes in triple-quoted
//! strings, and `${ ... }` interpolation depth across nested strings.
//!
//! These assert token-level behavior (the lexer produces tokens, not AST):
//! the fixed constructs must lex with the correct `TokenKind` and, critically,
//! yield **no** `TokenKind::Error` tokens.

use falcon_dart_parser::lexer::Lexer;
use falcon_syntax::token::TokenKind;

/// Lex `src` and return every token kind (trivia included).
fn kinds(src: &str) -> Vec<TokenKind> {
    Lexer::new(src)
        .tokenize()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

/// Assert the source lexes without producing any `Error` token.
fn assert_no_errors(src: &str) {
    let errs: Vec<_> = Lexer::new(src)
        .tokenize()
        .into_iter()
        .filter(|t| t.kind == TokenKind::Error)
        .collect();
    assert!(
        errs.is_empty(),
        "unexpected Error tokens in {src:?}: {errs:?}"
    );
}

/// The single non-trivia, non-EOF token kind produced for `src`.
fn only_kind(src: &str) -> TokenKind {
    let toks: Vec<_> = Lexer::new(src)
        .tokenize()
        .into_iter()
        .filter(|t| !t.is_trivia() && t.kind != TokenKind::Eof)
        .collect();
    assert_eq!(
        toks.len(),
        1,
        "expected exactly one token in {src:?}, got {toks:?}"
    );
    toks[0].kind.clone()
}

// ── (a) Doc comments: /** ... */ vs /* ... */ vs /**/ ──────────────────────────

#[test]
fn doc_block_comment_is_doc_comment() {
    // `/** ... */` is a documentation comment, not a plain block comment.
    let toks = Lexer::new("/** doc */").tokenize();
    assert_eq!(toks[0].kind, TokenKind::DocComment);
    assert_eq!(toks[0].text("/** doc */"), "/** doc */");
    assert_no_errors("/** doc */");
}

#[test]
fn plain_block_comment_stays_block_comment() {
    let toks = Lexer::new("/* block */").tokenize();
    assert_eq!(toks[0].kind, TokenKind::BlockComment);
}

#[test]
fn empty_block_comment_is_block_not_doc() {
    // `/**/` closes immediately — the trailing `/` is the terminator, so this
    // is an empty *block* comment, never a doc comment.
    let src = "/**/";
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].kind, TokenKind::BlockComment);
    assert_eq!(toks[0].len, 4);
    assert_no_errors(src);
}

#[test]
fn triple_star_open_is_doc_comment() {
    // `/***/` has a non-`/` char after `/**`, so it is a doc comment.
    let toks = Lexer::new("/***/").tokenize();
    assert_eq!(toks[0].kind, TokenKind::DocComment);
}

#[test]
fn doc_comment_preserves_nested_depth() {
    // Nested `/* */` inside a doc comment must not close it early.
    let src = "/** outer /* inner */ still doc */";
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].kind, TokenKind::DocComment);
    assert_eq!(toks[0].text(src), src);
    assert_no_errors(src);
}

#[test]
fn unterminated_doc_comment_is_error() {
    // Depth bookkeeping still governs termination.
    let toks = Lexer::new("/** never closed").tokenize();
    assert_eq!(toks[0].kind, TokenKind::Error);
}

// ── (b) Symbol literals: `#` lexes as Hash ─────────────────────────────────────

#[test]
fn hash_symbol_lexes_as_hash_plus_ident() {
    let src = "#foo";
    assert_eq!(
        kinds(src),
        vec![TokenKind::Hash, TokenKind::Ident, TokenKind::Eof]
    );
    assert_no_errors(src);
}

#[test]
fn bare_hash_is_hash_not_error() {
    assert_eq!(only_kind("#"), TokenKind::Hash);
    assert_no_errors("#");
}

// ── (c) Escapes in non-raw triple-quoted strings ───────────────────────────────

#[test]
fn triple_string_honors_escaped_quote() {
    // `'''foo\''''` — the `\'` is an escaped quote, so the string is `foo'`
    // and the final `'''` closes it. Without escape handling the lexer would
    // terminate early at the first run of three quotes and leave a stray quote.
    let src = "'''foo\\''''";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    // The whole source is one string literal — nothing trails it.
    assert_no_errors(src);
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].len, src.len());
}

#[test]
fn triple_string_escaped_backslash_then_close() {
    // `'''a\\'''` — escaped backslash, then the closing `'''`.
    let src = "'''a\\\\'''";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
}

#[test]
fn raw_triple_string_ignores_backslash() {
    // Raw strings: `\` is literal; `r'''a\'''` closes at the first `'''`.
    let src = "r'''a\\'''";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
}

// ── (d) `${ ... }` depth across nested strings ─────────────────────────────────

#[test]
fn interpolation_brace_inside_nested_single_string() {
    // `"${ m['}'] }"` — the `}` lives inside a nested single-quoted string and
    // must not close the interpolation early.
    let src = "\"${ m['}'] }\"";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].len, src.len());
}

#[test]
fn interpolation_brace_inside_nested_double_string() {
    // Nested double-quoted string inside a single-quoted host string.
    let src = "'${ f(\"}\") }'";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
}

#[test]
fn interpolation_nested_triple_string_with_brace() {
    let src = "\"${ g('''}''') }\"";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
}

#[test]
fn plain_interpolation_still_lexes() {
    let src = "\"${x + 1}\"";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
}

// ── (e) Suppression pinning: block-doc directive is a single DocComment token ───

#[test]
fn block_doc_ignore_lexes_as_single_doc_comment() {
    // A `/** falcon-ignore ... */` directive is now a DocComment token. This
    // documents the token-level fact the suppressions layer must guard against
    // (block-doc comments must not act as line-only directives).
    let src = "/** falcon-ignore lint/suspicious/avoid-dynamic: x */";
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].kind, TokenKind::DocComment);
    assert_eq!(toks[0].text(src), src);
    assert_no_errors(src);
}
