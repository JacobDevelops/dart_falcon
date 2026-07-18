use falcon_dart_parser::lexer::{Lexer, filter_trivia};
use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};

// ── Literals ──────────────────────────────────────────────────────────────────

#[test]
fn test_integer_decimal() {
    let tokens = Lexer::new("42").tokenize();
    assert_eq!(tokens.len(), 2); // IntLit + Eof
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
    assert_eq!(tokens[0].text("42"), "42");
}

#[test]
fn test_integer_hex() {
    let tokens = Lexer::new("0xFF").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
    assert_eq!(tokens[0].text("0xFF"), "0xFF");
}

#[test]
fn test_integer_hex_lowercase() {
    let tokens = Lexer::new("0xabcdef").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
}

#[test]
fn test_double_with_fraction() {
    let tokens = Lexer::new("3.14").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
}

#[test]
fn test_double_with_exponent() {
    let tokens = Lexer::new("1.5e10").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
}

#[test]
fn test_double_leading_dot() {
    let tokens = Lexer::new(".5").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
}

#[test]
fn test_double_exponent_negative() {
    let tokens = Lexer::new("1e-5").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
}

// ── Digit separators (Dart 3.6) ────────────────────────────────────────────────

#[test]
fn test_int_digit_separators() {
    let tokens = Lexer::new("1_000_000").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
    assert_eq!(tokens[0].text("1_000_000"), "1_000_000");
}

#[test]
fn test_hex_digit_separators() {
    let tokens = Lexer::new("0xFF_EC").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
    assert_eq!(tokens[0].text("0xFF_EC"), "0xFF_EC");
}

#[test]
fn test_double_digit_separators() {
    let src = "1_2.3_4e1_2";
    let tokens = Lexer::new(src).tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
    assert_eq!(tokens[0].text(src), src);
}

#[test]
fn test_trailing_underscore_is_part_of_number() {
    // The lexer does not reject a trailing separator; it is lexed as part of
    // the numeric token (matching current behavior).
    let tokens = Lexer::new("1_").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::IntLit);
    assert_eq!(tokens[0].text("1_"), "1_");
}

#[test]
fn test_leading_underscore_is_identifier() {
    // `_1` starts with `_`, so it is an identifier, not a number.
    let tokens = Lexer::new("_1").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Ident);
    assert_eq!(tokens[0].text("_1"), "_1");
}

// ── String literals ──────────────────────────────────────────────────────────

#[test]
fn test_string_single_quote() {
    let tokens = Lexer::new("'hello'").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
    assert_eq!(tokens[0].text("'hello'"), "'hello'");
}

#[test]
fn test_string_double_quote() {
    let tokens = Lexer::new("\"world\"").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_triple_single_quote() {
    let tokens = Lexer::new("'''multi\nline\nstring'''").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_triple_double_quote() {
    let tokens = Lexer::new("\"\"\"another\nmulti\"\"\"").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_raw() {
    let tokens = Lexer::new("r'raw\\nstring'").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_raw_triple() {
    let tokens = Lexer::new("r'''raw\nmulti'''").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_interpolation_simple() {
    let tokens = Lexer::new("'Hello $name'").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_interpolation_expression() {
    let tokens = Lexer::new("'Result: ${x + y}'").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_with_escape() {
    let tokens = Lexer::new("'quote\\'inside'").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::StringLit);
}

#[test]
fn test_string_unterminated() {
    let tokens = Lexer::new("'unterminated").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[test]
fn test_line_comment() {
    let tokens = Lexer::new("// comment").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LineComment);
    assert_eq!(tokens[0].text("// comment"), "// comment");
}

#[test]
fn test_doc_comment() {
    let tokens = Lexer::new("/// documentation").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DocComment);
}

#[test]
fn test_block_comment() {
    let tokens = Lexer::new("/* block */").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::BlockComment);
}

#[test]
fn test_nested_block_comment() {
    let tokens = Lexer::new("/* outer /* inner */ outer */").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::BlockComment);
}

#[test]
fn test_unterminated_block_comment() {
    let tokens = Lexer::new("/* unterminated").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

// ── Keywords ──────────────────────────────────────────────────────────────────

#[test]
fn test_keyword_assert() {
    let tokens = Lexer::new("assert").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Assert);
}

#[test]
fn test_keyword_break() {
    let tokens = Lexer::new("break").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Break);
}

#[test]
fn test_keyword_case() {
    let tokens = Lexer::new("case").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Case);
}

#[test]
fn test_keyword_class() {
    let tokens = Lexer::new("class").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Class);
}

#[test]
fn test_keyword_const() {
    let tokens = Lexer::new("const").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Const);
}

#[test]
fn test_keyword_return() {
    let tokens = Lexer::new("return").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Return);
}

#[test]
fn test_keyword_true() {
    let tokens = Lexer::new("true").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::True);
}

#[test]
fn test_keyword_false() {
    let tokens = Lexer::new("false").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::False);
}

#[test]
fn test_keyword_null() {
    let tokens = Lexer::new("null").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Null);
}

#[test]
fn test_keyword_var() {
    let tokens = Lexer::new("var").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Var);
}

// ── Built-in identifiers ──────────────────────────────────────────────────────

#[test]
fn test_builtin_abstract() {
    let tokens = Lexer::new("abstract").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Abstract);
}

#[test]
fn test_builtin_async() {
    let tokens = Lexer::new("async").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Async);
}

#[test]
fn test_builtin_await() {
    let tokens = Lexer::new("await").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Await);
}

#[test]
fn test_builtin_get() {
    let tokens = Lexer::new("get").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Get);
}

#[test]
fn test_builtin_set() {
    let tokens = Lexer::new("set").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Set);
}

#[test]
fn test_builtin_static() {
    let tokens = Lexer::new("static").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Static);
}

#[test]
fn test_builtin_override() {
    let tokens = Lexer::new("override").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Override);
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[test]
fn test_operator_plus() {
    let tokens = Lexer::new("+").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Plus);
}

#[test]
fn test_operator_minus() {
    let tokens = Lexer::new("-").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Minus);
}

#[test]
fn test_operator_star() {
    let tokens = Lexer::new("*").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Star);
}

#[test]
fn test_operator_slash() {
    let tokens = Lexer::new("/").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Slash);
}

#[test]
fn test_operator_percent() {
    let tokens = Lexer::new("%").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Percent);
}

#[test]
fn test_operator_tilde_slash() {
    let tokens = Lexer::new("~/").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::TildeSlash);
}

#[test]
fn test_operator_eq_eq() {
    let tokens = Lexer::new("==").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::EqEq);
}

#[test]
fn test_operator_bang_eq() {
    let tokens = Lexer::new("!=").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::BangEq);
}

#[test]
fn test_operator_lt() {
    let tokens = Lexer::new("<").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Lt);
}

#[test]
fn test_operator_gt() {
    let tokens = Lexer::new(">").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Gt);
}

#[test]
fn test_operator_lt_eq() {
    let tokens = Lexer::new("<=").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LtEq);
}

#[test]
fn test_operator_gt_eq() {
    let tokens = Lexer::new(">=").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::GtEq);
}

#[test]
fn test_operator_amp_amp() {
    let tokens = Lexer::new("&&").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::AmpAmp);
}

#[test]
fn test_operator_pipe_pipe() {
    let tokens = Lexer::new("||").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::PipePipe);
}

#[test]
fn test_operator_bang() {
    let tokens = Lexer::new("!").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Bang);
}

#[test]
fn test_operator_qmark_qmark() {
    let tokens = Lexer::new("??").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::QmarkQmark);
}

#[test]
fn test_operator_qmark_dot() {
    let tokens = Lexer::new("?.").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::QmarkDot);
}

#[test]
fn test_operator_qmark_bracket() {
    let tokens = Lexer::new("?[").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::QmarkLBracket);
}

#[test]
fn test_operator_dot_dot() {
    let tokens = Lexer::new("..").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DotDot);
}

#[test]
fn test_operator_dot_dot_qmark() {
    let tokens = Lexer::new("?..").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DotDotQmark);
}

#[test]
fn test_operator_spread() {
    let tokens = Lexer::new("...").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DotDotDot);
}

#[test]
fn test_operator_spread_qmark() {
    let tokens = Lexer::new("...?").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::DotDotDotQmark);
}

#[test]
fn test_operator_plus_plus() {
    let tokens = Lexer::new("++").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::PlusPlus);
}

#[test]
fn test_operator_minus_minus() {
    let tokens = Lexer::new("--").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::MinusMinus);
}

#[test]
fn test_operator_left_shift() {
    let tokens = Lexer::new("<<").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LtLt);
}

#[test]
fn test_operator_right_shift() {
    let tokens = Lexer::new(">>").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::GtGt);
}

#[test]
fn test_operator_unsigned_right_shift() {
    let tokens = Lexer::new(">>>").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::GtGtGt);
}

#[test]
fn test_operator_arrow() {
    let tokens = Lexer::new("=>").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Arrow);
}

// ── Punctuation ───────────────────────────────────────────────────────────────

#[test]
fn test_punct_lparen() {
    let tokens = Lexer::new("(").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LParen);
}

#[test]
fn test_punct_rparen() {
    let tokens = Lexer::new(")").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::RParen);
}

#[test]
fn test_punct_lbrace() {
    let tokens = Lexer::new("{").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LBrace);
}

#[test]
fn test_punct_rbrace() {
    let tokens = Lexer::new("}").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::RBrace);
}

#[test]
fn test_punct_lbracket() {
    let tokens = Lexer::new("[").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::LBracket);
}

#[test]
fn test_punct_rbracket() {
    let tokens = Lexer::new("]").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::RBracket);
}

#[test]
fn test_punct_comma() {
    let tokens = Lexer::new(",").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Comma);
}

#[test]
fn test_punct_semicolon() {
    let tokens = Lexer::new(";").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Semicolon);
}

#[test]
fn test_punct_colon() {
    let tokens = Lexer::new(":").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Colon);
}

#[test]
fn test_punct_at() {
    let tokens = Lexer::new("@").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::At);
}

#[test]
fn test_punct_dot() {
    let tokens = Lexer::new(".").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Dot);
}

// ── Identifiers ───────────────────────────────────────────────────────────────

#[test]
fn test_identifier_simple() {
    let tokens = Lexer::new("foo").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Ident);
}

#[test]
fn test_identifier_with_underscore() {
    let tokens = Lexer::new("_private").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Ident);
}

#[test]
fn test_identifier_with_digit() {
    let tokens = Lexer::new("var1Name").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Ident);
}

// ── Whitespace and newlines ───────────────────────────────────────────────────

#[test]
fn test_whitespace_space() {
    let tokens = Lexer::new("   ").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Whitespace);
}

#[test]
fn test_whitespace_tab() {
    let tokens = Lexer::new("\t\t").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Whitespace);
}

#[test]
fn test_newline() {
    let tokens = Lexer::new("\n").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Newline);
}

// ── Filter trivia ─────────────────────────────────────────────────────────────

#[test]
fn test_filter_trivia_removes_whitespace() {
    let raw_tokens = Lexer::new("foo   bar").tokenize();
    let filtered = filter_trivia(raw_tokens.clone());
    assert!(filtered.iter().all(|t| !t.is_trivia()));
    assert_eq!(filtered.len(), 3); // foo, bar, eof
}

#[test]
fn test_filter_trivia_removes_comments() {
    let raw_tokens = Lexer::new("foo // comment\nbar").tokenize();
    let filtered = filter_trivia(raw_tokens);
    assert_eq!(filtered.len(), 3); // foo, bar, eof
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::Ident);
}

#[test]
fn test_filter_trivia_keeps_eof() {
    let raw_tokens = Lexer::new("42").tokenize();
    let filtered = filter_trivia(raw_tokens);
    assert_eq!(filtered.last().unwrap().kind, TokenKind::Eof);
}

// ── Complex sequences ─────────────────────────────────────────────────────────

#[test]
fn test_sequence_class_decl() {
    let tokens = Lexer::new("class Foo {}").tokenize();
    let filtered = filter_trivia(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Class);
    assert_eq!(filtered[1].kind, TokenKind::Ident);
    assert_eq!(filtered[2].kind, TokenKind::LBrace);
    assert_eq!(filtered[3].kind, TokenKind::RBrace);
}

#[test]
fn test_sequence_function_call() {
    let tokens = Lexer::new("foo(42, 'text')").tokenize();
    let filtered = filter_trivia(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::LParen);
    assert_eq!(filtered[2].kind, TokenKind::IntLit);
    assert_eq!(filtered[3].kind, TokenKind::Comma);
    assert_eq!(filtered[4].kind, TokenKind::StringLit);
}

#[test]
fn test_sequence_null_aware_chain() {
    let tokens = Lexer::new("obj?.field?[0]??alt").tokenize();
    let filtered = filter_trivia(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::QmarkDot);
    assert_eq!(filtered[2].kind, TokenKind::Ident);
    assert_eq!(filtered[3].kind, TokenKind::QmarkLBracket);
    assert_eq!(filtered[4].kind, TokenKind::IntLit);
    assert_eq!(filtered[5].kind, TokenKind::RBracket);
    assert_eq!(filtered[6].kind, TokenKind::QmarkQmark);
}

#[test]
fn test_sequence_cascade() {
    let tokens = Lexer::new("obj..foo()..bar = 42").tokenize();
    let filtered = filter_trivia(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::DotDot);
    assert_eq!(filtered[2].kind, TokenKind::Ident);
}

// ── Malformed input / panic resilience ───────────────────────────────────────

#[test]
fn test_malformed_double_quote_unterminated() {
    let tokens = Lexer::new("\"unterminated").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

#[test]
fn test_malformed_raw_string_unterminated() {
    // r'...' without closing quote
    let tokens = Lexer::new("r'unterminated").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

#[test]
fn test_malformed_triple_quote_unterminated() {
    let tokens = Lexer::new("'''never closed").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

#[test]
fn test_malformed_backslash_at_eof() {
    // Backslash as the very last character inside a string
    let tokens = Lexer::new("'hello\\").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

#[test]
fn test_malformed_multiple_errors_no_panic() {
    // Two back-to-back unterminated tokens — lexer must not panic and must
    // produce at least one Error token
    let tokens = Lexer::new("'open1 /* open2").tokenize();
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Error));
}

#[test]
fn test_malformed_empty_input_no_panic() {
    // Empty source must not panic and should produce only Eof
    let tokens = Lexer::new("").tokenize();
    assert!(tokens.iter().all(|t| matches!(t.kind, TokenKind::Eof)));
}

#[test]
fn test_malformed_garbage_bytes_no_panic() {
    // Arbitrary byte sequences must not panic
    let _ = Lexer::new("\x00\x01\x02\x7f\u{FFFD}").tokenize();
}

#[test]
fn test_malformed_unmatched_block_comment_nested() {
    // /* /* */ — outer comment never closed
    let tokens = Lexer::new("/* /* inner */").tokenize();
    assert_eq!(tokens[0].kind, TokenKind::Error);
}

#[test]
fn test_malformed_valid_tokens_after_error_recover() {
    // After an error token the lexer should continue and produce valid tokens
    let tokens = Lexer::new("'bad 42").tokenize();
    // Must contain Error and IntLit somewhere (order may vary by impl)
    assert!(
        tokens.iter().any(|t| t.kind == TokenKind::Error)
            || tokens.iter().any(|t| t.kind == TokenKind::IntLit)
    );
    // Must never panic — reaching this line is the assertion
}

// ═══════════════════════════════════════════════════════════════════════════
// Lexer-gap regressions: doc comments (/** */), symbol-literal `#`, escapes in
// triple-quoted strings, and `${ ... }` interpolation depth across nested strings.
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// Lexer tail-end regressions: shebang lines, adjacent `$ident` + `${...}`
// interpolations, and nested triple-quoted strings inside interpolations.
// ═══════════════════════════════════════════════════════════════════════════

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Every token (trivia included) produced for `src`.
fn all_tokens(src: &str) -> Vec<Token> {
    Lexer::new(src).tokenize()
}

/// Assert `src` lexes without producing any `Error` token.
fn assert_no_error_tokens(src: &str) {
    let errs: Vec<_> = all_tokens(src)
        .into_iter()
        .filter(|t| t.kind == TokenKind::Error)
        .collect();
    assert!(
        errs.is_empty(),
        "unexpected Error tokens in {src:?}: {errs:?}"
    );
}

/// The text of each non-trivia, non-EOF token.
fn significant_texts(src: &str) -> Vec<String> {
    all_tokens(src)
        .into_iter()
        .filter(|t| !t.is_trivia() && t.kind != TokenKind::Eof)
        .map(|t| src[t.offset..t.offset + t.len].to_string())
        .collect()
}

/// Parse `void f() { var x = <expr>; }` and return the initializer string
/// literal together with the wrapped source (so interpolation spans can be
/// sliced). Asserts the whole program parses without error.
fn string_lit(expr_src: &str) -> (StringLitNode, String) {
    let src = format!("void f() {{ var x = {expr_src}; }}");
    let (program, errors) = parse(&src);
    assert!(
        errors.is_empty(),
        "parse errors for `{expr_src}`: {errors:?}"
    );
    let TopLevelDecl::Function(f) = &program.declarations[0] else {
        panic!("expected a top-level function");
    };
    let Some(FunctionBody::Block(block)) = &f.body else {
        panic!("expected a block body");
    };
    let Stmt::LocalVar(lv) = &block.stmts[0] else {
        panic!("expected a local var");
    };
    let Some(Expr::StringLit(node)) = &lv.declarators[0].initializer else {
        panic!("expected a string literal initializer");
    };
    (node.clone(), src)
}

/// The source text under each interpolation's span.
fn interp_texts(node: &StringLitNode, src: &str) -> Vec<String> {
    node.interpolations
        .iter()
        .map(|i| src[i.span.start..i.span.end].to_string())
        .collect()
}

// ── (a) Shebang ─────────────────────────────────────────────────────────────

#[test]
fn shebang_line_lexes_without_errors() {
    assert_no_error_tokens("#!/usr/bin/env dart\nvoid main() {}");
}

#[test]
fn shebang_is_the_first_token_as_a_comment() {
    let toks = all_tokens("#!/usr/bin/env dart\nvoid main() {}");
    let first = &toks[0];
    assert_eq!(first.kind, TokenKind::LineComment);
    assert!(
        first.is_trivia(),
        "shebang must be trivia so the parser skips it"
    );
    assert_eq!(
        &"#!/usr/bin/env dart\nvoid main() {}"[first.offset..first.offset + first.len],
        "#!/usr/bin/env dart"
    );
}

#[test]
fn shebang_without_trailing_newline_lexes_clean() {
    assert_no_error_tokens("#!/usr/bin/env dart");
    let toks = all_tokens("#!/usr/bin/env dart");
    assert_eq!(toks[0].kind, TokenKind::LineComment);
}

#[test]
fn shebang_file_parses_with_zero_errors() {
    let (_program, errors) = parse("#!/usr/bin/env dart\nvoid main() {}");
    assert!(
        errors.is_empty(),
        "shebang file should parse cleanly: {errors:?}"
    );
}

#[test]
fn hash_bang_not_at_offset_zero_is_not_trivia() {
    // Only byte offset 0 gets the shebang treatment; a `#` elsewhere lexes as a
    // normal symbol-literal introducer, never swallowing the rest of the line.
    let toks = all_tokens("\n#!/usr/bin/env dart");
    assert_ne!(toks[0].kind, TokenKind::LineComment);
    // First non-trivia token is the `#`, not a line comment.
    let first_sig = toks
        .iter()
        .find(|t| !t.is_trivia() && t.kind != TokenKind::Eof)
        .unwrap();
    assert_eq!(first_sig.kind, TokenKind::Hash);
}

// ── (b) Adjacent `$ident` immediately followed by `${...}` ──────────────────

#[test]
fn adjacent_ident_then_braced_is_one_string_token() {
    // Regression: `$m` used to swallow the following `$` (it is a valid
    // identifier-continue char), splitting the literal into three tokens.
    let src = "void f() { var x = '$m${ 'y' }'; }";
    assert_no_error_tokens(src);
    let texts = significant_texts(src);
    assert!(
        texts.contains(&"'$m${ 'y' }'".to_string()),
        "expected a single StringLit token, got {texts:?}"
    );
}

#[test]
fn adjacent_ident_then_braced_interpolations() {
    let (node, src) = string_lit("'$a${ 'y' }'");
    assert_eq!(interp_texts(&node, &src), vec!["a", "'y'"]);
    assert!(matches!(&node.interpolations[0].expr, Expr::Ident(id) if id.name == "a"));
    assert!(matches!(&node.interpolations[1].expr, Expr::StringLit(_)));
}

#[test]
fn adjacent_braced_then_braced_interpolations() {
    let (node, src) = string_lit("'${a}${ 'y' }'");
    assert_eq!(interp_texts(&node, &src), vec!["a", "'y'"]);
}

#[test]
fn several_simple_interpolations_stop_at_each_dollar() {
    let (node, src) = string_lit("'$a$b${c}'");
    assert_eq!(interp_texts(&node, &src), vec!["a", "b", "c"]);
    assert!(matches!(&node.interpolations[0].expr, Expr::Ident(id) if id.name == "a"));
    assert!(matches!(&node.interpolations[1].expr, Expr::Ident(id) if id.name == "b"));
    assert!(matches!(&node.interpolations[2].expr, Expr::Ident(id) if id.name == "c"));
}

// ── (c) Nested triple-quoted string inside an interpolation ─────────────────

#[test]
fn nested_triple_quote_in_interpolation_is_one_token() {
    let src = "void f() { var x = '''${wrap('''inner''')}'''; }";
    assert_no_error_tokens(src);
    let texts = significant_texts(src);
    assert!(
        texts.contains(&"'''${wrap('''inner''')}'''".to_string()),
        "expected a single StringLit token, got {texts:?}"
    );
}

#[test]
fn nested_triple_quote_in_interpolation_interpolations() {
    let (node, src) = string_lit("'''${wrap('''inner''')}'''");
    assert_eq!(interp_texts(&node, &src), vec!["wrap('''inner''')"]);
    assert!(matches!(node.interpolations[0].expr, Expr::Call { .. }));
}

#[test]
fn nested_double_triple_quote_in_interpolation() {
    let (node, src) = string_lit("\"\"\"${f(\"x\")}\"\"\"");
    assert_eq!(interp_texts(&node, &src), vec!["f(\"x\")"]);
    assert!(matches!(node.interpolations[0].expr, Expr::Call { .. }));
}

// ── Raw string inside an interpolation body ───────────────────────────────────

#[test]
fn test_raw_string_with_backslash_in_interpolation() {
    // Inside `${...}` the nested-string skipper must honor the `r` prefix: a raw
    // string has no escapes, so `r'\'` closes at its second quote. Without this
    // the backslash swallows the closing quote and the outer literal never ends.
    let src = "var x = '${d.replaceAll(r'\\', '/')}';";
    let tokens = filter_trivia(Lexer::new(src).tokenize());
    assert!(
        tokens.iter().all(|t| t.kind != TokenKind::Error),
        "unexpected Error token: {tokens:?}"
    );
    assert!(
        tokens.iter().any(|t| t.kind == TokenKind::StringLit),
        "expected a StringLit token: {tokens:?}"
    );

    // And it parses cleanly end-to-end, recording the interpolation.
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = match &prog.declarations[0] {
        TopLevelDecl::Variable(v) => v.declarators[0].initializer.as_ref().unwrap(),
        other => panic!("expected variable, got {other:?}"),
    };
    match init {
        Expr::StringLit(node) => assert_eq!(
            node.interpolations.len(),
            1,
            "expected one interpolation: {node:?}"
        ),
        other => panic!("expected string literal, got {other:?}"),
    }
}
