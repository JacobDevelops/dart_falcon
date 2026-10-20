//! Checked-in crash and complexity witnesses, exercised on every test run.
use falcon_dart_parser::parse;
use falcon_syntax::cst::visit::Visitor;
use falcon_syntax::cst::{DartType, Expr, Pattern};

struct Accessors;
impl<'a> Visitor<'a> for Accessors {}

fn exercise_corpus(source: &str) {
    let parsed = parse(source);
    assert!(parsed.tree.is_lossless(source));
    let root = parsed.tree.root(source);
    assert_eq!(root.full_span().start, 0);
    assert_eq!(root.full_span().end, source.len());

    // Accepted expression chains must not create an unbounded default visitor stack.
    Accessors.visit_program(parsed.tree.program(source));

    // Include recovery/orphan nodes, not only nodes reachable through Program's
    // typed declarations. Walking each cast exercises required-child accessors
    // even when malformed input interrupts the enclosing declaration.
    for node in root.descendants() {
        let span = node.span();
        assert!(span.start <= span.end && span.end <= source.len());
        assert_eq!(node.text(), &source[span.start..span.end]);
        for token in node
            .children_with_tokens()
            .filter_map(|child| child.into_token())
        {
            assert_eq!(token.text(), &source[token.span().start..token.span().end]);
        }
        if let Some(expr) = Expr::cast(node) {
            let _ = expr.span();
            Accessors.visit_expr(expr);
        }
        if let Some(pattern) = Pattern::cast(node) {
            let _ = pattern.span();
            Accessors.visit_pattern(pattern);
        }
        if let Some(ty) = DartType::cast(node) {
            let _ = ty.is_nullable();
            Accessors.visit_dart_type(ty);
        }
    }
}

macro_rules! corpus_case {
    ($($name:ident),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                exercise_corpus(include_str!(concat!("fixtures/adversarial/", stringify!($name), ".dart")));
            }
        )+
    };
}

corpus_case!(
    nested_map_set,
    map_depth26,
    mixed_brace_elements,
    malformed_lambda_fallback,
    malformed_block_lambda,
    malformed_arrow_chain,
    nested_patterns,
    nested_types,
    switch_arm_depth,
    unclosed_annotations,
    annotated_declarations,
    binary_chain,
    postfix_chain,
    as_is_chain,
);
