use falcon_dart_parser::{lexer::Lexer, parse};
use falcon_syntax::cst::{CascadeCall, CascadeInvoke, NodeKind};
use falcon_syntax::token::TokenKind;

#[test]
fn unclosed_interpolations_preserve_following_declarations() {
    for literal in [
        "'${value'",
        "\"${value\"",
        "'''${value'''",
        "\"\"\"${value\"\"\"",
        "'${value",
    ] {
        let source = format!("var broken = {literal};\nvoid retained() {{}}\n");
        let tokens = Lexer::new(&source).tokenize();
        let error = tokens
            .iter()
            .find(|token| token.kind == TokenKind::Error)
            .unwrap();
        assert!(error.offset() + error.len() <= source.find('\n').unwrap());
        assert!(tokens.iter().any(|token| token.text(&source) == "retained"));
        let parsed = parse(&source);
        assert!(!parsed.errors.is_empty());
        assert!(parsed.tree.is_lossless(&source));
        assert!(parsed.tree.root(&source).descendants().any(|node| {
            node.kind() == NodeKind::FunctionDecl && node.text().contains("retained")
        }));
    }
}

#[test]
fn valid_multiline_interpolation_is_not_recovered_at_a_newline() {
    let source = "var value = '${\n  call('nested')\n}';\nvoid retained() {}";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(source));
    assert_eq!(
        parsed
            .tree
            .root(source)
            .descendants()
            .filter(|node| node.kind() == NodeKind::StringInterpolation)
            .count(),
        1
    );
}

#[test]
fn abandoned_member_parses_do_not_leak_diagnostics() {
    let source = "class C { method<T extends Object>(T value) {} }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(source));
    assert_eq!(
        parsed
            .tree
            .root(source)
            .descendants()
            .filter(|node| node.kind() == NodeKind::MethodDecl)
            .count(),
        1
    );
}

#[test]
fn failed_member_alternatives_report_only_committed_recovery() {
    let source = "class C { broken<; int retained; }";
    let parsed = parse(source);
    assert_eq!(parsed.errors.len(), 1, "{:?}", parsed.errors);
    assert_eq!(parsed.errors[0].message, "could not parse class member");
    assert!(parsed.tree.is_lossless(source));
    assert!(
        parsed
            .tree
            .root(source)
            .descendants()
            .any(|node| { node.kind() == NodeKind::FieldDecl && node.text().contains("retained") })
    );
}

#[test]
fn cascade_and_postfix_share_selector_grammar_without_changing_shapes() {
    for source in [
        "void f() { target.method<Map<String, List<int>>>(value).next?[index]!(arg); }",
        "void f() { target..method<Map<String, List<int>>>(value).next?[index]!(arg); }",
        "void f() { target..method()(arg)<Map<String, List<int>>>(value); }",
        "void f() { target..[index].method<int>(value)..field[index] = value; }",
    ] {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(source));
        for node in parsed.tree.root(source).descendants() {
            if let Some(call) = CascadeCall::cast(node) {
                assert_eq!(call.name().name(), "method");
                let generic = node.text().contains('<');
                assert_eq!(call.type_args().count(), usize::from(generic));
                assert_eq!(call.args().positional().count(), usize::from(generic));
            }
            if let Some(invoke) = CascadeInvoke::cast(node) {
                assert_eq!(invoke.args().positional().count(), 1);
                assert_eq!(
                    invoke.type_args().count(),
                    usize::from(node.text().contains('<'))
                );
            }
        }
    }
}

#[test]
fn malformed_selector_recovery_preserves_later_statements() {
    for expression in ["target.method<int>(value).", "target..method<int>(value)."] {
        let source = format!("void f() {{ {expression}; retained(); }}");
        let parsed = parse(&source);
        assert!(!parsed.errors.is_empty());
        assert!(parsed.tree.is_lossless(&source));
        assert!(
            parsed
                .tree
                .root(&source)
                .descendants()
                .any(|node| { node.kind() == NodeKind::CallExpr && node.text() == "retained()" })
        );
    }
}

#[test]
fn unclosed_interpolation_does_not_take_a_brace_from_a_later_string() {
    let source = "var s = \"${x\";\nvar t = \"}\";\nvoid main() {}";
    let parsed = parse(source);
    assert!(!parsed.errors.is_empty());
    assert!(parsed.tree.is_lossless(source));
    let root = parsed.tree.root(source);
    assert!(
        root.descendants().any(|node| {
            node.kind() == NodeKind::FunctionDecl && node.text() == "void main() {}"
        })
    );
    assert!(root.descendants().any(|node| {
        node.kind() == NodeKind::TopLevelVarDecl && node.text().contains("t = \"}\"")
    }));
}

#[test]
fn constructor_body_survives_successfully_parsed_initializer_brackets() {
    for prefix in ["f(0)", "(0)", "[0]", "{0}", "{0: 0}"] {
        let source = format!("class C {{ int x; C() : x = {prefix} ?? (y) {{ print(y); }} }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        let constructor = parsed
            .tree
            .root(&source)
            .descendants()
            .find_map(falcon_syntax::cst::ConstructorDecl::cast)
            .unwrap();
        assert!(constructor.body().is_some(), "{source}");
        assert_eq!(constructor.initializers().count(), 1);
        assert!(
            !parsed
                .tree
                .root(&source)
                .descendants()
                .any(|node| node.kind() == NodeKind::FuncExpr),
            "{source}"
        );
    }
}

#[test]
fn initializer_brackets_still_allow_nested_block_closures() {
    let source = "class C { int x; C() : x = f((y) { return y; }) ?? (z) { print(z); } }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(source));
    let constructor = parsed
        .tree
        .root(source)
        .descendants()
        .find_map(falcon_syntax::cst::ConstructorDecl::cast)
        .unwrap();
    assert!(constructor.body().is_some());
    assert_eq!(
        parsed
            .tree
            .root(source)
            .descendants()
            .filter(|node| node.kind() == NodeKind::FuncExpr)
            .count(),
        1
    );
}

#[test]
fn legacy_typedefs_discard_modern_alternative_diagnostics() {
    for source in [
        "typedef Future<void> Callback();",
        "typedef List<(int, int)> Callback();",
    ] {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(source));
        assert_eq!(
            parsed
                .tree
                .root(source)
                .descendants()
                .filter(|node| node.kind() == NodeKind::LegacyTypedef)
                .count(),
            1
        );
    }
}
