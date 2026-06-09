use jdlint_dart_parser::lexer::{filter_trivia, Lexer};
use jdlint_syntax::token::TokenKind;

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
    assert_eq!(
        filtered.len(),
        3
    ); // foo, bar, eof
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
