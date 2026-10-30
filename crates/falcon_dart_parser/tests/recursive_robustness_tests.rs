use falcon_dart_parser::{ParseResult, parse};
use falcon_syntax::cst::visit::Visitor;
use falcon_syntax::cst::*;

mod common;

struct Walk;
impl Visitor<'_> for Walk {}

fn assert_depth_limited(source: &str) {
    let parsed = parse(source);
    assert_eq!(
        parsed
            .errors
            .iter()
            .filter(|error| error.message == "nesting too deep")
            .count(),
        1,
        "expected one depth diagnostic: {:?}",
        parsed.errors,
    );
    assert!(parsed.tree.is_lossless(source));
    Walk.visit_program(parsed.tree.program(source));
}

fn first_pattern<'a>(parsed: &'a ParseResult, source: &'a str) -> Pattern<'a> {
    let Stmt::Switch(switch) = common::body_stmts(parsed.tree.program(source))
        .next()
        .unwrap()
    else {
        panic!("expected switch");
    };
    let SwitchCaseKind::Pattern(case) = switch.cases().next().unwrap().cases().next().unwrap()
    else {
        panic!("expected pattern case");
    };
    case.pattern()
}

#[test]
fn nested_pattern_forms_stop_at_the_depth_limit() {
    for (open, close) in [
        ("[", "]"),
        ("(", ")"),
        ("(field: ", ")"),
        ("{'key': ", "}"),
        ("C(field: ", ")"),
        ("C(:", ")"),
    ] {
        let pattern = format!("{}var value{}", open.repeat(512), close.repeat(512));
        assert_depth_limited(&format!(
            "void f() {{ switch (value) {{ case {pattern}: break; }} }}"
        ));
    }
}

#[test]
fn long_pattern_chains_parse_and_walk_without_diagnostics() {
    for pattern in [
        format!("{}0", "0 || ".repeat(100_000)),
        format!("{}0", "0 && ".repeat(100_000)),
        format!("var value{}", "?!".repeat(50_000)),
        format!("var value{}", " as int".repeat(100_000)),
        format!("'value'{}", ".field".repeat(100_000)),
    ] {
        let source = format!("void f() {{ switch (value) {{ case {pattern}: break; }} }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        Walk.visit_program(parsed.tree.program(&source));
    }
}

#[test]
fn nested_type_forms_stop_at_the_depth_limit() {
    for (open, close) in [
        ("List<", ">"),
        ("(", ",)"),
        ("({", " field})"),
        ("void Function(", ")"),
        ("void Function<T extends ", ">()"),
    ] {
        let ty = format!("{}int{}", open.repeat(512), close.repeat(512));
        assert_depth_limited(&format!("typedef F = {ty};"));
    }
    assert_depth_limited(&format!("typedef F = int{};", " Function()".repeat(512)));
}

#[test]
fn old_style_function_formals_stop_at_the_depth_limit() {
    let formal = format!("{}int value{}", "callback(".repeat(512), ")".repeat(512));
    assert_depth_limited(&format!("void f({formal}) {{}}"));
}

#[test]
fn nested_expression_forms_skip_past_the_depth_limit() {
    for (open, close) in [("[", "]"), ("(", ")"), ("{", "}"), ("{'key': ", "}")] {
        let source = format!(
            "void f() {{ var value = {}0{}; print(value); }}",
            open.repeat(10_000),
            close.repeat(10_000)
        );
        assert_depth_limited(&source);
        let parsed = parse(&source);
        assert_eq!(parsed.errors.len(), 1, "{open}: {:?}", parsed.errors);
        assert_eq!(
            common::body_stmts(parsed.tree.program(&source)).count(),
            2,
            "{open}"
        );
    }
}

#[test]
fn pattern_chain_accessors_preserve_associativity_and_postfix_order() {
    let source = "void f() { switch (value) { case 1 || 2 || 3: break; } }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let Pattern::LogicalOr(outer) = first_pattern(&parsed, source) else {
        panic!("expected logical-or pattern");
    };
    assert!(matches!(outer.right(), Pattern::Literal(_)));
    let Pattern::LogicalOr(inner) = outer.left() else {
        panic!("expected left-associated logical-or pattern");
    };
    assert!(matches!(inner.left(), Pattern::Literal(_)));
    assert!(matches!(inner.right(), Pattern::Literal(_)));

    let source = "void f() { switch (value) { case var binding?! as int: break; } }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let Pattern::Cast(cast) = first_pattern(&parsed, source) else {
        panic!("expected cast pattern");
    };
    assert!(matches!(cast.cast_type(), DartType::Named(_)));
    let Pattern::NullAssert(assert) = cast.inner() else {
        panic!("expected null-assert pattern");
    };
    let Pattern::NullCheck(check) = assert.inner() else {
        panic!("expected null-check pattern");
    };
    let Pattern::Variable(binding) = check.inner() else {
        panic!("expected variable pattern");
    };
    assert_eq!(binding.name().name(), "binding");
}

#[test]
fn bounded_nested_type_accessors_keep_every_layer() {
    let source = format!("typedef F = {}int{};", "List<".repeat(24), ">".repeat(24));
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::TypeAlias(alias) =
        parsed.tree.program(&source).declarations().next().unwrap()
    else {
        panic!("expected type alias");
    };
    let mut ty = alias.aliased();
    for _ in 0..24 {
        let DartType::Named(named) = ty else {
            panic!("expected named type");
        };
        assert_eq!(named.segments().next().unwrap().name(), "List");
        assert_eq!(named.type_args().len(), 1);
        ty = named.type_args().next().unwrap();
    }
    let DartType::Named(named) = ty else {
        panic!("expected innermost int");
    };
    assert_eq!(named.segments().next().unwrap().name(), "int");

    let source = format!("typedef F = int{};", " Function()".repeat(24));
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::TypeAlias(alias) =
        parsed.tree.program(&source).declarations().next().unwrap()
    else {
        panic!("expected type alias");
    };
    let mut ty = alias.aliased();
    for _ in 0..24 {
        let DartType::Function(function) = ty else {
            panic!("expected curried function layer");
        };
        assert!(function.params().is_empty());
        ty = function.return_type().unwrap();
    }
    assert!(matches!(ty, DartType::Named(_)));
}

#[test]
fn expression_chain_depth_does_not_leak_between_patterns() {
    let case = format!("case == value{}: break;", ".field".repeat(20));
    let source = format!("void f() {{ switch (value) {{ {} }} }}", case.repeat(32));
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(&source));
    Walk.visit_program(parsed.tree.program(&source));
}

#[test]
fn empty_parameter_recovers_through_the_comma_and_keeps_the_body() {
    let source = "void f(int a, , int b) { return; }";
    let parsed = parse(source);
    assert_eq!(parsed.errors.len(), 1, "{:?}", parsed.errors);
    assert!(parsed.tree.is_lossless(source));
    assert!(matches!(
        common::body_stmts(parsed.tree.program(source)).next(),
        Some(Stmt::Return(_))
    ));
}

#[test]
fn relational_expression_depth_does_not_leak_into_logical_siblings() {
    let operand = format!("{}value", "value | ".repeat(80));
    let source =
        format!("void f() {{ switch (value) {{ case < {operand} && < {operand}: break; }} }}");
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let Pattern::LogicalAnd(pattern) = first_pattern(&parsed, &source) else {
        panic!("expected logical-and pattern");
    };
    assert!(matches!(pattern.left(), Pattern::Relational(_)));
    assert!(matches!(pattern.right(), Pattern::Relational(_)));
    Walk.visit_program(parsed.tree.program(&source));
}

#[test]
fn overflow_diagnostic_does_not_disable_later_type_and_pattern_lists() {
    let source = format!(
        "void overflow() {{ var value = {}true; }}
         void g(int a) {{}}
         typedef G<T extends Object> = List<T>;
         typedef R = (int, {{String name}});
         void patterns(value) {{
           switch (value) {{
             case [var a, var b]: break;
             case C(field: var c): break;
           }}
         }}",
        "!".repeat(512),
    );
    let parsed = parse(&source);
    assert_eq!(
        parsed
            .errors
            .iter()
            .filter(|error| error.message == "nesting too deep")
            .count(),
        1,
    );
    assert!(parsed.tree.is_lossless(&source));
    let mut declarations = parsed.tree.program(&source).declarations();
    let function = declarations
        .clone()
        .find_map(|declaration| match declaration {
            TopLevelDecl::Function(function) if function.name().name() == "g" => Some(function),
            _ => None,
        })
        .expect("function after overflow is preserved");
    let parameters = function.params().unwrap();
    assert_eq!(parameters.params().len(), 1);
    let parameter = parameters.params().next().unwrap();
    assert_eq!(parameter.name().name(), "a");
    assert!(matches!(parameter.param_type(), Some(DartType::Named(_))));
    let generic = declarations
        .clone()
        .find_map(|declaration| match declaration {
            TopLevelDecl::TypeAlias(alias) if alias.name().name() == "G" => Some(alias),
            _ => None,
        })
        .expect("generic alias after overflow is preserved");
    assert_eq!(generic.type_params().len(), 1);
    assert!(generic.type_params().next().unwrap().bound().is_some());
    let DartType::Named(list) = generic.aliased() else {
        panic!("expected List type");
    };
    assert_eq!(list.type_args().len(), 1);
    let record = declarations
        .clone()
        .find_map(|declaration| match declaration {
            TopLevelDecl::TypeAlias(alias) if alias.name().name() == "R" => Some(alias.aliased()),
            _ => None,
        })
        .expect("record alias after overflow is preserved");
    let DartType::Record(record) = record else {
        panic!("expected record type");
    };
    assert_eq!(record.positional().len(), 1);
    assert_eq!(record.named().len(), 1);
    let patterns = declarations
        .find_map(|declaration| match declaration {
            TopLevelDecl::Function(function) if function.name().name() == "patterns" => {
                Some(function)
            }
            _ => None,
        })
        .expect("pattern function after overflow is preserved");
    let Some(FunctionBody::Block(body)) = patterns.body() else {
        panic!("expected function body");
    };
    let Stmt::Switch(switch) = body.stmts().next().unwrap() else {
        panic!("expected switch");
    };
    let mut cases = switch.cases();
    let SwitchCaseKind::Pattern(case) = cases.next().unwrap().cases().next().unwrap() else {
        panic!("expected list case");
    };
    let Pattern::List(list) = case.pattern() else {
        panic!("expected list pattern");
    };
    assert_eq!(list.elements().len(), 2);
    let SwitchCaseKind::Pattern(case) = cases.next().unwrap().cases().next().unwrap() else {
        panic!("expected object case");
    };
    let Pattern::Object(object) = case.pattern() else {
        panic!("expected object pattern");
    };
    assert_eq!(object.fields().len(), 1);
    Walk.visit_program(parsed.tree.program(&source));
}

#[test]
fn nested_pool_parsing_preserves_deep_input_on_a_small_caller_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            falcon_dart_parser::with_stack(|| {
                let source = format!(
                    "typedef Deep = {}int{};",
                    "List<".repeat(96),
                    ">".repeat(96)
                );
                for _ in 0..8 {
                    let parsed = parse(&source);
                    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
                    assert!(parsed.tree.is_lossless(&source));
                    Walk.visit_program(parsed.tree.program(&source));
                }
                assert_depth_limited(&format!(
                    "typedef TooDeep = {}int{};",
                    "List<".repeat(512),
                    ">".repeat(512)
                ));
            })
            .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
