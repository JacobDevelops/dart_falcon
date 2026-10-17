use super::{InterpolationRegion, lex_region, visit_interpolations};
use crate::lexer::Lexer;
use crate::parser::Parser;
use falcon_syntax::LanguageVersion;
use falcon_syntax::cst::{NodeKind, Span, SyntaxTree};
use falcon_syntax::token::TokenKind;

fn expression_span(source: &str, expression: &str) -> Span {
    let start = source.find(expression).unwrap();
    Span::new(start, start + expression.len())
}

#[test]
fn retains_nested_tokens_with_absolute_offsets_and_trivia() {
    let source = r#"'prefix ${ /* outer */ "nested ${value + 1}" } suffix'"#;
    let tree = crate::lex_tree(source);
    assert!(tree.is_lossless(source));
    assert!(
        tree.tokens(source)
            .any(|token| token.kind() == TokenKind::BlockComment)
    );
    let inner_span = expression_span(source, "value + 1");
    assert!(
        tree.tokens(source)
            .any(|token| token.kind() == TokenKind::Ident
                && token.text() == "value"
                && token.offset() == inner_span.start)
    );
    let inner = lex_region(source, inner_span);
    assert_eq!(inner.first().unwrap().offset(), inner_span.start);
    assert_eq!(inner.last().unwrap().offset(), inner_span.end);
    assert_eq!(inner.last().unwrap().kind, TokenKind::Eof);
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
    let expression = parser.parse_expr();
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
    assert_eq!(expression.span(), Span::new(0, source.len()));
}

#[test]
fn raw_and_escaped_dollars_do_not_create_interpolation_regions() {
    for source in [r#"r'${ignored}'"#, r#"'\${ignored}'"#] {
        let tree = crate::lex_tree(source);
        assert!(
            !tree
                .root(source)
                .descendants()
                .any(|node| node.kind() == NodeKind::StringInterpolation)
        );
        let mut count = 0;
        visit_interpolations(source, 0, |_| count += 1);
        assert_eq!(count, 0);
    }
}

#[test]
fn adjacent_literals_and_simple_identifiers_keep_distinct_regions() {
    let source = r#"'$left$right' '${third}'"#;
    let tokens = Lexer::new(source).tokenize();
    let first = tokens
        .iter()
        .find(|token| token.kind == TokenKind::StringLit)
        .unwrap();
    let mut identifiers = Vec::with_capacity(2);
    visit_interpolations(first.text(source), first.offset(), |region| {
        if let InterpolationRegion::Identifier(span) = region {
            identifiers.push(&source[span.start..span.end]);
        }
    });
    assert_eq!(identifiers, ["left", "right"]);
    let tree = crate::lex_tree(source);
    assert_eq!(
        tree.identifier_tokens(source)
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        ["left", "right", "third"]
    );
    let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
    assert_ne!(parser.parse_expr().kind(), NodeKind::ErrorExpr);
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
}

#[test]
fn malformed_outer_strings_preserve_lexer_tokens() {
    for source in [r#"'${missing'"#, r#"'unterminated"#] {
        let tree = crate::lex_tree(source);
        let original = Lexer::new(source).tokenize();
        assert_eq!(tree.tokens(source).len(), original.len());
        for (retained, original) in tree.tokens(source).zip(original) {
            assert_eq!(retained.kind(), original.kind);
            assert_eq!(retained.offset(), original.offset());
            assert_eq!(retained.len(), original.len());
        }
    }
}

#[test]
fn empty_braced_interpolation_remains_silent() {
    let source = "'${}'";
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
    parser.parse_string_lit();
    assert!(parser.errors.is_empty());
    assert!(!parser.events.iter().any(|event| matches!(
        event,
        falcon_syntax::cst::Event::Start {
            kind: NodeKind::StringInterpolation,
            ..
        }
    )));
}

#[test]
fn interpolation_diagnostics_match_standalone_expression_offsets() {
    let source = "'prefix ${1 + } suffix'";
    let span = expression_span(source, "1 + ");
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
    parser.parse_string_lit();
    assert!(!parser.events.iter().any(|event| matches!(
        event,
        falcon_syntax::cst::Event::Start {
            kind: NodeKind::StringInterpolation,
            ..
        }
    )));
    let standalone = Lexer::new(&source[span.start..span.end])
        .tokenize()
        .into_iter()
        .map(|token| {
            falcon_syntax::token::Token::new(token.kind, token.offset() + span.start, token.len())
        })
        .collect::<Vec<_>>();
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
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
    parser.parse_block();
    assert!(parser.errors.is_empty(), "{:?}", parser.errors);
    let tree = SyntaxTree::build(source, &tokens, parser.events);
    let declaration = tree
        .root(source)
        .descendants()
        .find_map(falcon_syntax::cst::LocalVarDecl::cast)
        .unwrap();
    assert_eq!(declaration.span().start, source.find("final").unwrap());
    assert_eq!(declaration.full_span().start, source.find("///").unwrap());
    assert_eq!(declaration.full_span().end, declaration.span().end);
}

#[test]
fn identifier_queries_include_simple_and_nested_interpolations() {
    let source = r#"'$left ${"$nested ${right}"}'"#;
    let tree = crate::lex_tree(source);
    let identifiers = tree
        .identifier_tokens(source)
        .map(|token| (token.offset(), token.text()))
        .collect::<Vec<_>>();
    assert_eq!(
        identifiers,
        [
            (source.find("left").unwrap(), "left"),
            (source.find("nested").unwrap(), "nested"),
            (source.find("right").unwrap(), "right"),
        ]
    );
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
        assert_eq!(closing.parent().kind(), NodeKind::StringInterpolation);
    }
}

#[test]
fn lexer_only_entry_retains_deep_interpolation_tokens() {
    let depth = 512;
    let source = format!("{}Name{}", "'${".repeat(depth), "}'".repeat(depth));
    let tree = crate::lex_tree(&source);
    assert!(tree.is_lossless(&source));
    assert_eq!(
        tree.root(&source)
            .descendants()
            .filter(|node| node.kind() == NodeKind::StringInterpolation)
            .count(),
        depth
    );
    assert_eq!(
        tree.identifier_tokens(&source)
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        ["Name"]
    );
}
