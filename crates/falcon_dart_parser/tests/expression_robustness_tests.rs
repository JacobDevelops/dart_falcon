use falcon_dart_parser::parse;
use std::sync::mpsc;
use std::time::Duration;

fn parse_with_timeout(source: String) -> falcon_dart_parser::ParseResult {
    let (send, recv) = mpsc::channel();
    std::thread::spawn(move || send.send(parse(&source)).unwrap());
    recv.recv_timeout(Duration::from_secs(10))
        .expect("adversarial parse must finish within ten seconds")
}

#[test]
fn nested_braces_parse_once() {
    let source = format!("var value = {}0{};", "{'key': ".repeat(40), "}".repeat(40));
    let parsed = parse_with_timeout(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
}

#[test]
fn malformed_lambda_defaults_are_not_reparsed() {
    let source = format!("var value = {}+{};", "(a = ".repeat(40), ") {}".repeat(40));
    let parsed = parse_with_timeout(source);
    assert!(!parsed.errors.is_empty());
}

#[test]
fn contextual_parameter_modifiers_remain_expression_identifiers() {
    let source =
        "void f() { var a = (required,); var b = (covariant,); var c = (required + covariant); }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(source));
}

#[test]
fn long_expression_chains_parse_and_walk_without_diagnostics() {
    use falcon_syntax::cst::visit::Visitor;
    struct Walker;
    impl Visitor<'_> for Walker {}
    for suffix in [" + x", ".field", " as T", " is T"] {
        let source = format!(
            "void f() {{ var value = [x{}, after()]; }} var after = 1;",
            suffix.repeat(100_000)
        );
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{suffix}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        assert_eq!(parsed.tree.program(&source).declarations().len(), 2);
        Walker.visit_program(parsed.tree.program(&source));
    }
}

#[test]
fn unresolved_brace_overflow_recovers_as_a_typed_map_entry() {
    use falcon_syntax::cst::{Expr, MapElement};
    let source = format!(
        "var value = <String, int>{{{}'key': 1}};",
        "if (condition) ".repeat(200)
    );
    let parsed = parse(&source);
    assert!(!parsed.errors.is_empty());
    let map = parsed
        .tree
        .root(&source)
        .descendants()
        .filter_map(Expr::cast)
        .find_map(|expr| match expr {
            Expr::Map(map) => Some(map),
            _ => None,
        })
        .expect("two type arguments resolve the literal as a map");
    let mut element = map.elements().next().unwrap();
    let mut wrappers = 0;
    loop {
        match element {
            MapElement::If(if_element) => {
                wrappers += 1;
                element = if_element.then_entry();
            }
            MapElement::Entry(entry) => {
                assert!(wrappers > 0);
                assert!(matches!(entry.key(), Expr::Error(_)));
                assert!(matches!(entry.value(), Expr::Error(_)));
                break;
            }
            _ => panic!("expected nested map-if elements and a recovery entry"),
        }
    }
}
