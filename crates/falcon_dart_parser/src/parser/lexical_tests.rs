use super::{InterpolationRegion, lex_syntax, visit_interpolations};
use crate::lexer::Lexer;
use crate::parser::Parser;
use falcon_syntax::LanguageVersion;
use falcon_syntax::ast::{Expr, Span, Stmt};
use falcon_syntax::token::{SyntaxStore, Token, TokenKind};

fn expression_span(source: &str, expression: &str) -> Span {
    let start = source.find(expression).unwrap();
    Span::new(start, start + expression.len())
}

#[test]
fn retains_nested_streams_with_absolute_offsets_and_trivia() {
    let source = r#"'prefix ${ /* outer */ "nested ${value + 1}" } suffix'"#;
    let syntax = lex_syntax(source);
    let outer_span = expression_span(source, " /* outer */ \"nested ${value + 1}\" ");
    let outer = syntax.interpolation(outer_span).unwrap();
    assert!(
        outer
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::BlockComment)
    );
    let inner_span = expression_span(source, "value + 1");
    let inner = outer.interpolation(inner_span).unwrap();
    assert_eq!(inner.tokens().first().unwrap().offset(), inner_span.start);
    assert_eq!(inner.tokens().last().unwrap().offset(), inner_span.end);
    assert_eq!(inner.tokens().last().unwrap().kind, TokenKind::Eof);

    let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
    let expression = parser.parse_expr();
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
    assert_eq!(*expression.span(), Span::new(0, source.len()));
}

#[test]
fn raw_and_escaped_dollars_do_not_retain_expression_streams() {
    for source in [r#"r'${ignored}'"#, r#"'\${ignored}'"#] {
        let syntax = lex_syntax(source);
        assert!(
            syntax
                .interpolation(expression_span(source, "ignored"))
                .is_none()
        );
        let mut count = 0;
        visit_interpolations(source, 0, |_| count += 1);
        assert_eq!(count, 0);
    }
}

#[test]
fn adjacent_literals_and_simple_identifiers_keep_distinct_regions() {
    let source = r#"'$left$right' '${third}'"#;
    let syntax = lex_syntax(source);
    assert!(
        syntax
            .interpolation(expression_span(source, "third"))
            .is_some()
    );
    let first = syntax
        .tokens()
        .iter()
        .find(|token| token.kind == TokenKind::StringLit)
        .unwrap();
    let mut identifiers = Vec::new();
    visit_interpolations(first.text(source), first.offset(), |region| {
        if let InterpolationRegion::Identifier(span) = region {
            identifiers.push(&source[span.start..span.end]);
        }
    });
    assert_eq!(identifiers, ["left", "right"]);
    let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
    assert!(!matches!(parser.parse_expr(), Expr::Error { .. }));
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
}

#[test]
fn malformed_outer_strings_preserve_lexer_tokens() {
    for source in [r#"'${missing'"#, r#"'unterminated"#] {
        let syntax = lex_syntax(source);
        let original = Lexer::new(source).tokenize();
        assert_eq!(syntax.tokens().len(), original.len());
        for (retained, original) in syntax.tokens().iter().zip(original) {
            assert_eq!(retained.kind, original.kind);
            assert_eq!(retained.offset(), original.offset());
            assert_eq!(retained.len(), original.len());
        }
    }
}

#[test]
fn empty_braced_interpolation_remains_silent() {
    let source = "'${}'";
    let syntax = lex_syntax(source);
    assert!(syntax.interpolation(Span::new(3, 3)).is_some());
    let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
    let string = parser.parse_string_lit();
    assert!(string.interpolations.is_empty());
    assert!(parser.errors.is_empty());
}

#[test]
fn interpolation_diagnostics_match_standalone_expression_offsets() {
    let source = "'prefix ${1 + } suffix'";
    let span = expression_span(source, "1 + ");
    let syntax = lex_syntax(source);
    let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
    let string = parser.parse_string_lit();
    assert!(string.interpolations.is_empty());

    let mut tokens = Lexer::new(&source[span.start..span.end]).tokenize();
    for token in &mut tokens {
        *token = Token::new(
            token.kind,
            token.offset().checked_add(span.start).unwrap(),
            token.len(),
        );
    }
    let standalone = SyntaxStore::new(tokens);
    let mut baseline = Parser::new(&standalone, source, LanguageVersion::CURRENT);
    baseline.parse_expr();
    assert!(!baseline.errors.is_empty());
    let diagnostics = |errors: Vec<crate::parser::ParseError>| {
        errors
            .into_iter()
            .map(|error| (error.message, error.offset))
            .collect::<Vec<_>>()
    };
    assert_eq!(diagnostics(parser.errors), diagnostics(baseline.errors));
}

#[test]
fn local_documentation_extends_only_full_span() {
    let source = "{ /** old block */ /// attached line\n final value = 1; }";
    let syntax = lex_syntax(source);
    let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
    let block = parser.parse_block();
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
    let Stmt::LocalVar(declaration) = &block.stmts[0] else {
        panic!("expected local variable declaration");
    };
    assert_eq!(declaration.span.start, source.find("final").unwrap());
    assert_eq!(declaration.full_span.start, source.find("///").unwrap());
    assert_eq!(declaration.full_span.end, declaration.span.end);
}

#[test]
fn identifier_queries_include_simple_and_nested_interpolations() {
    let source = r#"'$left ${"$nested ${right}"}'"#;
    let syntax = lex_syntax(source);
    let mut identifiers = syntax
        .identifier_tokens()
        .map(|token| (token.offset(), token.text(source)))
        .collect::<Vec<_>>();
    identifiers.sort_unstable();
    assert_eq!(
        identifiers,
        [
            (source.find("left").unwrap(), "left"),
            (source.find("nested").unwrap(), "nested"),
            (source.find("right").unwrap(), "right"),
        ]
    );
    assert!(
        !syntax
            .significant_tokens()
            .any(|token| { matches!(token.text(source), "left" | "nested") })
    );
}

#[test]
fn lexer_only_entry_retains_manifest_interpolation_identifiers() {
    let source = "name: workspace\nvalue: '$Simple ${Braced} ${\"$Nested\"}'\n";
    let syntax = crate::parser::lex_syntax(source);
    let identifiers = syntax
        .identifier_tokens()
        .map(|token| token.text(source))
        .collect::<Vec<_>>();
    assert_eq!(
        identifiers,
        ["name", "workspace", "value", "Simple", "Braced", "Nested"]
    );
    assert_eq!(syntax.tokens().last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn lexical_tree_retains_manifest_interpolation_identifiers() {
    let source = "name: workspace\nvalue: '$Simple ${Braced} ${\"$Nested\"} ${show} ${get} ${ { 'key': hide } }'\n";
    let tree = crate::lex_tree(source);
    assert!(tree.is_lossless(source));
    assert!(tree.program(source).declarations().is_empty());
    assert_eq!(
        tree.identifier_tokens(source)
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        [
            "name",
            "workspace",
            "value",
            "Simple",
            "Braced",
            "Nested",
            "show",
            "get",
            "hide"
        ]
    );
    assert_eq!(tree.tokens(source).last().unwrap().kind(), TokenKind::Eof);
}

#[test]
fn lexical_tree_uses_scanned_interpolation_boundaries() {
    for source in [r"x: '${\{}'", r"x: '${a\}b}'"] {
        let tree = crate::lex_tree(source);
        assert!(tree.is_lossless(source));
        let closing = tree
            .tokens(source)
            .find(|token| {
                token.kind() == TokenKind::RBrace && token.offset() == source.rfind('}').unwrap()
            })
            .unwrap();
        assert_eq!(
            closing.parent().kind(),
            falcon_syntax::cst::NodeKind::StringInterpolation
        );
    }
}

#[test]
fn lexer_only_entry_retains_deep_interpolation_streams() {
    let depth = 512;
    let source = format!("{}Name{}", "'${".repeat(depth), "}'".repeat(depth));
    let syntax = crate::parser::lex_syntax(&source);
    let mut nested = &syntax;
    for level in 0..depth {
        nested = nested
            .interpolation(Span::new((level + 1) * 3, source.len() - (level + 1) * 2))
            .expect("every nested expression must retain its lexical store");
    }
    assert_eq!(nested.tokens()[0].text(&source), "Name");
    assert_eq!(
        syntax
            .identifier_tokens()
            .map(|token| token.text(&source))
            .collect::<Vec<_>>(),
        ["Name"]
    );
}
