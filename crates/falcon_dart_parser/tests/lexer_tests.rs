use falcon_dart_parser::lexer::Lexer;
use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;
use falcon_syntax::token::{Token, TokenKind};

fn significant_tokens(tokens: Vec<Token>) -> Vec<Token> {
    tokens
        .into_iter()
        .filter(|token| !token.is_trivia())
        .collect()
}

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

#[test]
fn test_exponent_digit_separators() {
    for src in [".5e1_0", "0.5e1_0", ".5e-1_0", "1e+1_0"] {
        let tokens = Lexer::new(src).tokenize();
        assert_eq!(tokens.len(), 2, "{src}");
        assert_eq!(tokens[0].kind, TokenKind::DoubleLit);
        assert_eq!(tokens[0].text(src), src);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }
}

#[test]
fn test_interpolation_comments_ignore_closing_braces() {
    for src in ["'${a // }\n}'", "'${a /* } */}'"] {
        let tokens = Lexer::new(src).tokenize();
        assert_eq!(tokens.len(), 2, "{src:?}: {tokens:?}");
        assert_eq!(tokens[0].kind, TokenKind::StringLit, "{src:?}");
        assert_eq!(tokens[0].text(src), src);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }
}

#[test]
fn test_unterminated_interpolation_and_comment_consume_source() {
    for src in ["'${ /* x", "'${ 'x", "/* x"] {
        let tokens = Lexer::new(src).tokenize();
        assert_eq!(tokens.len(), 2, "{src:?}: {tokens:?}");
        assert_eq!(tokens[0].kind, TokenKind::Error, "{src:?}");
        assert_eq!(tokens[0].offset(), 0);
        assert_eq!(tokens[0].len(), src.len());
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }
}

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

#[test]
fn test_keyword_operator_punctuation_table() {
    let cases = [
        ("test_keyword_assert", "assert", TokenKind::Assert),
        ("test_keyword_break", "break", TokenKind::Break),
        ("test_keyword_case", "case", TokenKind::Case),
        ("keyword_catch", "catch", TokenKind::Catch),
        ("test_keyword_class", "class", TokenKind::Class),
        ("test_keyword_const", "const", TokenKind::Const),
        ("keyword_continue", "continue", TokenKind::Continue),
        ("keyword_default", "default", TokenKind::Default),
        ("keyword_do", "do", TokenKind::Do),
        ("keyword_else", "else", TokenKind::Else),
        ("keyword_enum", "enum", TokenKind::Enum),
        ("keyword_extends", "extends", TokenKind::Extends),
        ("test_keyword_false", "false", TokenKind::False),
        ("keyword_final", "final", TokenKind::Final),
        ("keyword_finally", "finally", TokenKind::Finally),
        ("keyword_for", "for", TokenKind::For),
        ("keyword_if", "if", TokenKind::If),
        ("keyword_in", "in", TokenKind::In),
        ("keyword_is", "is", TokenKind::Is),
        ("keyword_new", "new", TokenKind::New),
        ("test_keyword_null", "null", TokenKind::Null),
        ("keyword_rethrow", "rethrow", TokenKind::Rethrow),
        ("test_keyword_return", "return", TokenKind::Return),
        ("keyword_super", "super", TokenKind::Super),
        ("keyword_switch", "switch", TokenKind::Switch),
        ("keyword_this", "this", TokenKind::This),
        ("keyword_throw", "throw", TokenKind::Throw),
        ("test_keyword_true", "true", TokenKind::True),
        ("keyword_try", "try", TokenKind::Try),
        ("test_keyword_var", "var", TokenKind::Var),
        ("keyword_void", "void", TokenKind::Void),
        ("keyword_while", "while", TokenKind::While),
        ("keyword_with", "with", TokenKind::With),
        ("test_builtin_abstract", "abstract", TokenKind::Abstract),
        ("keyword_as", "as", TokenKind::As),
        ("keyword_base", "base", TokenKind::Base),
        ("keyword_covariant", "covariant", TokenKind::Covariant),
        ("keyword_deferred", "deferred", TokenKind::Deferred),
        ("keyword_dynamic", "dynamic", TokenKind::Dynamic),
        ("keyword_export", "export", TokenKind::Export),
        ("keyword_extension", "extension", TokenKind::Extension),
        ("keyword_external", "external", TokenKind::External),
        ("keyword_factory", "factory", TokenKind::Factory),
        ("keyword_Function", "Function", TokenKind::Function),
        ("test_builtin_get", "get", TokenKind::Get),
        ("keyword_hide", "hide", TokenKind::Hide),
        ("keyword_implements", "implements", TokenKind::Implements),
        ("keyword_import", "import", TokenKind::Import),
        ("keyword_interface", "interface", TokenKind::Interface),
        ("keyword_late", "late", TokenKind::Late),
        ("keyword_library", "library", TokenKind::Library),
        ("keyword_mixin", "mixin", TokenKind::Mixin),
        ("keyword_operator", "operator", TokenKind::Operator),
        ("keyword_part", "part", TokenKind::Part),
        ("keyword_required", "required", TokenKind::Required),
        ("keyword_sealed", "sealed", TokenKind::Sealed),
        ("test_builtin_set", "set", TokenKind::Set),
        ("keyword_show", "show", TokenKind::Show),
        ("test_builtin_static", "static", TokenKind::Static),
        ("keyword_type", "type", TokenKind::Type),
        ("keyword_typedef", "typedef", TokenKind::Typedef),
        ("test_builtin_async", "async", TokenKind::Async),
        ("test_builtin_await", "await", TokenKind::Await),
        ("keyword_sync", "sync", TokenKind::Sync),
        ("keyword_yield", "yield", TokenKind::Yield),
        ("keyword_when", "when", TokenKind::When),
        ("keyword_on", "on", TokenKind::On),
        ("test_builtin_override", "override", TokenKind::Override),
        ("test_operator_plus", "+", TokenKind::Plus),
        ("test_operator_minus", "-", TokenKind::Minus),
        ("test_operator_star", "*", TokenKind::Star),
        ("test_operator_slash", "/", TokenKind::Slash),
        ("test_operator_percent", "%", TokenKind::Percent),
        ("test_operator_tilde_slash", "~/", TokenKind::TildeSlash),
        ("test_operator_eq_eq", "==", TokenKind::EqEq),
        ("test_operator_bang_eq", "!=", TokenKind::BangEq),
        ("test_operator_lt", "<", TokenKind::Lt),
        ("test_operator_gt", ">", TokenKind::Gt),
        ("test_operator_lt_eq", "<=", TokenKind::LtEq),
        ("test_operator_gt_eq", ">=", TokenKind::GtEq),
        ("test_operator_amp_amp", "&&", TokenKind::AmpAmp),
        ("test_operator_pipe_pipe", "||", TokenKind::PipePipe),
        ("test_operator_bang", "!", TokenKind::Bang),
        ("test_operator_qmark_qmark", "??", TokenKind::QmarkQmark),
        ("test_operator_qmark_dot", "?.", TokenKind::QmarkDot),
        (
            "test_operator_qmark_bracket",
            "?[",
            TokenKind::QmarkLBracket,
        ),
        ("test_operator_dot_dot", "..", TokenKind::DotDot),
        ("test_operator_dot_dot_qmark", "?..", TokenKind::DotDotQmark),
        ("test_operator_spread", "...", TokenKind::DotDotDot),
        (
            "test_operator_spread_qmark",
            "...?",
            TokenKind::DotDotDotQmark,
        ),
        ("test_operator_plus_plus", "++", TokenKind::PlusPlus),
        ("test_operator_minus_minus", "--", TokenKind::MinusMinus),
        ("test_operator_left_shift", "<<", TokenKind::LtLt),
        ("test_operator_right_shift", ">>", TokenKind::GtGt),
        (
            "test_operator_unsigned_right_shift",
            ">>>",
            TokenKind::GtGtGt,
        ),
        ("test_operator_arrow", "=>", TokenKind::Arrow),
        ("test_punct_lparen", "(", TokenKind::LParen),
        ("test_punct_rparen", ")", TokenKind::RParen),
        ("test_punct_lbrace", "{", TokenKind::LBrace),
        ("test_punct_rbrace", "}", TokenKind::RBrace),
        ("test_punct_lbracket", "[", TokenKind::LBracket),
        ("test_punct_rbracket", "]", TokenKind::RBracket),
        ("test_punct_comma", ",", TokenKind::Comma),
        ("test_punct_semicolon", ";", TokenKind::Semicolon),
        ("test_punct_colon", ":", TokenKind::Colon),
        ("test_punct_at", "@", TokenKind::At),
        ("test_punct_dot", ".", TokenKind::Dot),
    ];
    for (witness, src, expected) in cases {
        assert_eq!(only_kind(src), expected, "{witness}: {src:?}");
    }
}

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

#[test]
fn test_significant_tokens_exclude_whitespace() {
    let raw_tokens = Lexer::new("foo   bar").tokenize();
    let filtered = significant_tokens(raw_tokens);
    assert!(filtered.iter().all(|t| !t.is_trivia()));
    assert_eq!(filtered.len(), 3); // foo, bar, eof
}

#[test]
fn test_significant_tokens_exclude_comments() {
    let raw_tokens = Lexer::new("foo // comment\nbar").tokenize();
    let filtered = significant_tokens(raw_tokens);
    assert_eq!(filtered.len(), 3); // foo, bar, eof
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::Ident);
}

#[test]
fn test_significant_tokens_keep_eof() {
    let raw_tokens = Lexer::new("42").tokenize();
    let filtered = significant_tokens(raw_tokens);
    assert_eq!(filtered.last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn test_sequence_class_decl() {
    let tokens = Lexer::new("class Foo {}").tokenize();
    let filtered = significant_tokens(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Class);
    assert_eq!(filtered[1].kind, TokenKind::Ident);
    assert_eq!(filtered[2].kind, TokenKind::LBrace);
    assert_eq!(filtered[3].kind, TokenKind::RBrace);
}

#[test]
fn test_sequence_function_call() {
    let tokens = Lexer::new("foo(42, 'text')").tokenize();
    let filtered = significant_tokens(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::LParen);
    assert_eq!(filtered[2].kind, TokenKind::IntLit);
    assert_eq!(filtered[3].kind, TokenKind::Comma);
    assert_eq!(filtered[4].kind, TokenKind::StringLit);
}

#[test]
fn test_sequence_null_aware_chain() {
    let tokens = Lexer::new("obj?.field?[0]??alt").tokenize();
    let filtered = significant_tokens(tokens);
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
    let filtered = significant_tokens(tokens);
    assert_eq!(filtered[0].kind, TokenKind::Ident);
    assert_eq!(filtered[1].kind, TokenKind::DotDot);
    assert_eq!(filtered[2].kind, TokenKind::Ident);
}

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

// Lexer-gap regressions: doc comments (/** */), symbol-literal `#`, escapes in
// triple-quoted strings, and `${ ... }` interpolation depth across nested strings.

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
    toks[0].kind
}

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
    assert_eq!(toks[0].len(), 4);
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
    assert_eq!(toks[0].len(), src.len());
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

#[test]
fn interpolation_brace_inside_nested_single_string() {
    // `"${ m['}'] }"` — the `}` lives inside a nested single-quoted string and
    // must not close the interpolation early.
    let src = "\"${ m['}'] }\"";
    assert_eq!(only_kind(src), TokenKind::StringLit);
    assert_no_errors(src);
    let toks = Lexer::new(src).tokenize();
    assert_eq!(toks[0].len(), src.len());
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

// Lexer tail-end regressions: shebang lines, adjacent `$ident` + `${...}`
// interpolations, and nested triple-quoted strings inside interpolations.

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
        .map(|t| src[t.offset()..t.offset() + t.len()].to_string())
        .collect()
}

#[path = "common/string_lit.rs"]
mod common_string;
use common_string::string_lit;

/// The source text under each interpolation's span.
fn interp_texts(node: StringLitNode<'_>, src: &str) -> Vec<String> {
    node.interpolations()
        .map(|i| src[i.span().start..i.span().end].to_string())
        .collect()
}

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
        &"#!/usr/bin/env dart\nvoid main() {}"[first.offset()..first.offset() + first.len()],
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
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("#!/usr/bin/env dart\nvoid main() {}");
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
    let src = "void f() { var x = '$a${ 'y' }'; }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(src));
    assert_eq!(interp_texts(node, src), vec!["a", " 'y' "]);
    assert!(
        matches!(node.interpolations().next().unwrap().expr(), Expr::Ident(id) if id.name() == "a")
    );
    assert!(matches!(
        node.interpolations().nth(1).unwrap().expr(),
        Expr::StringLit(_)
    ));
}

#[test]
fn adjacent_braced_then_braced_interpolations() {
    let src = "void f() { var x = '${a}${ 'y' }'; }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(src));
    assert_eq!(interp_texts(node, src), vec!["a", " 'y' "]);
}

#[test]
fn several_simple_interpolations_stop_at_each_dollar() {
    let src = "void f() { var x = '$a$b${c}'; }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(src));
    assert_eq!(interp_texts(node, src), vec!["a", "b", "c"]);
    assert!(
        matches!(node.interpolations().next().unwrap().expr(), Expr::Ident(id) if id.name() == "a")
    );
    assert!(
        matches!(node.interpolations().nth(1).unwrap().expr(), Expr::Ident(id) if id.name() == "b")
    );
    assert!(
        matches!(node.interpolations().nth(2).unwrap().expr(), Expr::Ident(id) if id.name() == "c")
    );
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
    let src = "void f() { var x = '''${wrap('''inner''')}'''; }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(src));
    assert_eq!(interp_texts(node, src), vec!["wrap('''inner''')"]);
    assert!(matches!(
        node.interpolations().next().unwrap().expr(),
        Expr::Call(_)
    ));
}

#[test]
fn nested_double_triple_quote_in_interpolation() {
    let src = "void f() { var x = \"\"\"${f(\"x\")}\"\"\"; }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(src));
    assert_eq!(interp_texts(node, src), vec!["f(\"x\")"]);
    assert!(matches!(
        node.interpolations().next().unwrap().expr(),
        Expr::Call(_)
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// UTF-8 handling: leading BOM stripping and ASCII-only identifiers.
// ═══════════════════════════════════════════════════════════════════════════

// ── Leading UTF-8 BOM (U+FEFF) ─────────────────────────────────────────────────

#[test]
fn leading_bom_is_stripped_no_error() {
    // Real Dart accepts a leading BOM as clean source; the lexer must skip it
    // rather than emit an Error token at offset 0.
    let src = "\u{FEFF}void main() {}";
    assert_no_error_tokens(src);
    let toks = all_tokens(src);
    assert_ne!(toks[0].kind, TokenKind::Error);
    // First token begins just past the 3-byte BOM.
    assert_eq!(toks[0].offset(), '\u{FEFF}'.len_utf8());
}

#[test]
fn bom_file_parses_with_zero_errors() {
    let falcon_dart_parser::ParseResult { errors, .. } = parse("\u{FEFF}void main() {}");
    assert!(errors.is_empty(), "BOM file should parse clean: {errors:?}");
}

#[test]
fn bom_not_at_offset_zero_is_error() {
    // Only a leading BOM is stripped; an interior U+FEFF stays illegal.
    let toks = all_tokens("void\u{FEFF}main");
    assert!(
        toks.iter().any(|t| t.kind == TokenKind::Error),
        "interior BOM must remain an Error token: {toks:?}"
    );
}

// ── ASCII-only identifiers (Dart illegal_character) ────────────────────────────

#[test]
fn non_ascii_identifier_char_is_error() {
    // Dart identifiers are ASCII only; a CJK char is `illegal_character`.
    let toks = all_tokens("var 変数 = 1;");
    assert!(
        toks.iter().any(|t| t.kind == TokenKind::Error),
        "non-ASCII identifier chars must lex as Error: {toks:?}"
    );
}

#[test]
fn non_ascii_char_is_single_error_token_no_panic() {
    // Each non-ASCII char becomes one Error token spanning its UTF-8 bytes.
    let toks = all_tokens("é");
    let errs: Vec<_> = toks.iter().filter(|t| t.kind == TokenKind::Error).collect();
    assert_eq!(errs.len(), 1, "expected a single Error token: {toks:?}");
    assert_eq!(errs[0].len(), 'é'.len_utf8());
}

#[test]
fn ascii_identifiers_still_lex() {
    assert_eq!(only_kind("foo"), TokenKind::Ident);
    assert_no_error_tokens("foo _bar $baz name1");
}

// ── Raw string inside an interpolation body ───────────────────────────────────

#[test]
fn test_raw_string_with_backslash_in_interpolation() {
    // Inside `${...}` the nested-string skipper must honor the `r` prefix: a raw
    // string has no escapes, so `r'\'` closes at its second quote. Without this
    // the backslash swallows the closing quote and the outer literal never ends.
    let src = "var x = '${d.replaceAll(r'\\', '/')}';";
    let tokens = significant_tokens(Lexer::new(src).tokenize());
    assert!(
        tokens.iter().all(|t| t.kind != TokenKind::Error),
        "unexpected Error token: {tokens:?}"
    );
    assert!(
        tokens.iter().any(|t| t.kind == TokenKind::StringLit),
        "expected a StringLit token: {tokens:?}"
    );

    // And it parses cleanly end-to-end, recording the interpolation.
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = match prog.declarations().first().unwrap() {
        TopLevelDecl::Variable(v) => v.declarators().first().unwrap().initializer().unwrap(),
        other => panic!("expected variable, got {other:?}"),
    };
    match init {
        Expr::StringLit(node) => assert_eq!(
            node.interpolations().count(),
            1,
            "expected one interpolation: {node:?}"
        ),
        other => panic!("expected string literal, got {other:?}"),
    }
}

#[test]
fn test_bom_then_shebang_is_script_directive() {
    // Dart grammar: `FEFF? SCRIPT_TAG? ...` — a shebang may follow a BOM.
    let src = "\u{FEFF}#!/usr/bin/env dart\nvoid main() {}\n";
    let tokens = Lexer::new(src).tokenize();
    assert!(
        !tokens.iter().any(|t| t.kind == TokenKind::Error),
        "BOM + shebang must lex clean: {tokens:?}"
    );
}

#[test]
fn line_comments_end_at_every_dart_line_ending() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = format!(
            "/// docs{newline}void documented() {{}}{newline}// ordinary{newline}void visible() {{}}"
        );
        let tokens = Lexer::new(&source).tokenize();
        let doc = tokens
            .iter()
            .find(|token| token.kind == TokenKind::DocComment)
            .unwrap();
        assert_eq!(doc.text(&source), "/// docs");
        assert_eq!(
            tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Newline)
                .count(),
            3
        );
        let falcon_dart_parser::ParseResult { tree, errors } = parse(&source);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(tree.program(&source).declarations().len(), 2);
    }
}
