//! Minimal-repro tests for inline function-type parsing gaps closed in the
//! `types` group:
//!   (a) a bare special type (`dynamic`, `void`, `Never`) as the return type of
//!       an inline function type — `dynamic Function(String)`.
//!   (b) generic inline function types with their own type params —
//!       `void Function<T>(T x)`, `void Function<T extends num>(T)`.
//!   (c) function-typed fields inside object patterns ride on (a)/(b).
//!
//! Each test asserts ZERO parse errors and the relevant syntax shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

mod common;
#[path = "common/only_class.rs"]
mod common_class;
use common_class::only_class;

/// Return the aliased type of a single top-level `typedef`.
fn typedef_aliased(prog: Program<'_>) -> DartType<'_> {
    match prog.declarations().next().unwrap() {
        TopLevelDecl::TypeAlias(t) => t.aliased(),
        other => panic!("expected typedef, got {other:?}"),
    }
}

fn as_function(ty: DartType<'_>) -> FunctionType<'_> {
    match ty {
        DartType::Function(f) => f,
        other => panic!("expected function type, got {other:?}"),
    }
}

/// The type of the sole positional formal of a top-level `void f(<param>) {}`.
fn sole_param_type(prog: Program<'_>) -> DartType<'_> {
    let func = match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let param = func.params().unwrap().params().next().unwrap();
    assert_eq!(param.kind(), ParamKind::Positional);
    param.param_type().expect("param should carry a type")
}

fn parse_source(source: &str) -> falcon_dart_parser::ParseResult {
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    parsed
}

#[test]
fn function_type_parameters_preserve_required_and_optional_shapes() {
    for (source, expected) in [
        (
            "typedef F = void Function(int, [String?]);",
            vec![(true, false), (false, false)],
        ),
        (
            "typedef F = void Function(int, {required String label, bool? enabled});",
            vec![(true, false), (true, true), (false, true)],
        ),
        (
            "typedef void F(int value, [String? label]);",
            vec![(true, false), (false, false)],
        ),
    ] {
        let parsed = parse_source(source);
        let aliased = typedef_aliased(parsed.tree.program(source));
        let function = as_function(aliased);
        let actual: Vec<_> = function
            .params()
            .map(|parameter| (parameter.is_required(), parameter.is_named()))
            .collect();
        assert_eq!(actual, expected, "{source}");
    }
}

#[test]
fn nullable_function_parameters_in_every_type_position() {
    for parameters in [
        "Map<String, dynamic> data, {String? tablePrefix}",
        "Map<String, dynamic> data, [String? tablePrefix]",
        "void Function({String? label}) callback, {String? tablePrefix}",
    ] {
        let ty = format!("Future Function({parameters})");
        for source in [
            format!("typedef F = {ty};"),
            format!("class C {{ {ty}? field; }}"),
            format!("List<{ty}> f() => [];"),
            format!("{ty} f({ty} callback) => callback;"),
            format!("bool f(Object value) => value is {ty};"),
            format!("bool f(Object value) => value is {ty} ? true : false;"),
            format!("Object f(Object value) => value as {ty};"),
            format!(
                "bool f(Object value) => switch (value) {{ {ty} callback => true, _ => false }};"
            ),
        ] {
            let falcon_dart_parser::ParseResult { errors, .. } = parse(&source);
            assert!(errors.is_empty(), "{source}: {errors:?}");
        }
    }
}

#[test]
fn is_function_type_restores_conditional_question_mark() {
    let source = "bool f(Object value) => value is void Function({String? label}) ? true : false;";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let program = parsed.tree.program(source);
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("expected function");
    };
    let Some(FunctionBody::Arrow(body)) = function.body() else {
        panic!("expected expression body");
    };
    let expr = body.expr();
    let Expr::Conditional(conditional) = expr else {
        panic!("expected conditional, got {expr:?}");
    };
    let Expr::Is(type_test) = conditional.condition() else {
        panic!("expected type test");
    };
    let function_type = as_function(type_test.dart_type());
    assert!(!function_type.is_nullable());
    assert!(
        matches!(function_type.params().next().unwrap().param_type(), DartType::Named(named) if named.is_nullable())
    );
}

#[test]
fn positional_record_type_fields_preserve_optional_source_names() {
    let source = "typedef R = (int first, String, bool last);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let DartType::Record(record) = aliased else {
        panic!("expected record type");
    };
    assert_eq!(record.positional().len(), 3);
    let names: Vec<_> = record
        .positional_names()
        .map(|name| name.map(|name| name.name()))
        .collect();
    assert_eq!(names, [Some("first"), None, Some("last")]);
}

#[test]
fn function_type_parameters_preserve_annotations() {
    for source in [
        "typedef F = void Function(@deprecated int value);",
        "typedef void F(@deprecated int value);",
    ] {
        let parsed = parse_source(source);
        let aliased = typedef_aliased(parsed.tree.program(source));
        let function = as_function(aliased);
        let parameter = function.params().next().unwrap();
        assert_eq!(parameter.annotations().len(), 1);
        assert_eq!(
            parameter
                .annotations()
                .next()
                .unwrap()
                .name()
                .next()
                .unwrap()
                .name(),
            "deprecated"
        );
    }
}

#[test]
fn dynamic_function_return_in_typedef() {
    let source = "typedef F = dynamic Function(int);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    assert!(
        matches!(f.return_type(), Some(DartType::Dynamic(_))),
        "return type should be dynamic, got {:?}",
        f.return_type()
    );
    assert_eq!(f.params().len(), 1);
    assert!(matches!(
        f.params().next().unwrap().param_type(),
        DartType::Named(_)
    ));
}

#[test]
fn dynamic_function_return_as_param() {
    let source = "void f(dynamic Function(String) cb) {}";
    let parsed = parse_source(source);
    let ty = sole_param_type(parsed.tree.program(source));
    let f = as_function(ty);
    assert!(matches!(f.return_type(), Some(DartType::Dynamic(_))));
    assert_eq!(f.params().len(), 1);
}

#[test]
fn void_function_return_in_typedef() {
    let source = "typedef F = void Function(int);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    assert!(matches!(f.return_type(), Some(DartType::Void(_))));
}

#[test]
fn never_function_return_in_typedef() {
    let source = "typedef F = Never Function(int);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    // `Never` is a contextual name, so it surfaces as a named return type.
    match f.return_type() {
        Some(DartType::Named(n)) => assert_eq!(n.segments().next().unwrap().name(), "Never"),
        other => panic!("expected named `Never` return, got {other:?}"),
    }
}

#[test]
fn generic_function_type_in_typedef() {
    let source = "typedef F = void Function<T>(T x);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    assert!(matches!(f.return_type(), Some(DartType::Void(_))));
    assert_eq!(f.type_params().len(), 1);
    assert_eq!(f.type_params().next().unwrap().name().name(), "T");
    assert!(f.type_params().next().unwrap().bound().is_none());
    assert_eq!(f.params().len(), 1);
    assert_eq!(f.params().next().unwrap().name().unwrap().name(), "x");
}

#[test]
fn generic_function_type_bounded_as_param() {
    let source = "void f(void Function<T extends num>(T) cb) {}";
    let parsed = parse_source(source);
    let ty = sole_param_type(parsed.tree.program(source));
    let f = as_function(ty);
    assert_eq!(f.type_params().len(), 1);
    assert_eq!(f.type_params().next().unwrap().name().name(), "T");
    match f.type_params().next().unwrap().bound() {
        Some(DartType::Named(n)) => assert_eq!(n.segments().next().unwrap().name(), "num"),
        other => panic!("expected `num` bound, got {other:?}"),
    }
    assert_eq!(f.params().len(), 1);
}

#[test]
fn bare_generic_function_type_in_typedef() {
    let source = "typedef F = Function<T>(T x);";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    assert!(f.return_type().is_none());
    assert_eq!(f.type_params().len(), 1);
    assert_eq!(f.type_params().next().unwrap().name().name(), "T");
}

/// Extract the pattern of the first `case` label of a `switch` in a function body.
fn first_case_pattern(prog: Program<'_>) -> Pattern<'_> {
    let sw = common::body_stmts(prog)
        .find_map(|s| match s {
            Stmt::Switch(sw) => Some(sw),
            _ => None,
        })
        .expect("expected a switch statement");
    match sw.cases().next().unwrap().cases().next().unwrap() {
        SwitchCaseKind::Pattern(p) => p.pattern(),
        other => panic!("expected a pattern case, got {other:?}"),
    }
}

#[test]
fn object_pattern_dynamic_function_field() {
    let source = "void f() { switch (x) { case Typedef(:dynamic Function(Invocation) noSuchMethod): break; } }";
    let parsed = parse_source(source);
    let pat = first_case_pattern(parsed.tree.program(source));
    let obj = match pat {
        Pattern::Object(o) => o,
        other => panic!("expected object pattern, got {other:?}"),
    };
    assert_eq!(obj.fields().len(), 1);
    let field = obj.fields().next().unwrap();
    assert_eq!(field.name().name(), "noSuchMethod");
    match field.pattern().expect("field binds a pattern") {
        Pattern::Variable(variable) => {
            assert_eq!(variable.name().name(), "noSuchMethod");
            let f = as_function(variable.type_().expect("variable is typed"));
            assert!(matches!(f.return_type(), Some(DartType::Dynamic(_))));
        }
        other => panic!("expected typed variable pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_generic_function_field() {
    let source = "void f() { switch (x) { case Foo(:void Function<T>(T) cb): break; } }";
    let parsed = parse_source(source);
    let pat = first_case_pattern(parsed.tree.program(source));
    let obj = match pat {
        Pattern::Object(o) => o,
        other => panic!("expected object pattern, got {other:?}"),
    };
    match obj
        .fields()
        .next()
        .unwrap()
        .pattern()
        .expect("field binds a pattern")
    {
        Pattern::Variable(variable) => {
            let f = as_function(variable.type_().expect("variable is typed"));
            assert_eq!(f.type_params().len(), 1);
            assert_eq!(f.type_params().next().unwrap().name().name(), "T");
        }
        other => panic!("expected typed variable pattern, got {other:?}"),
    }
}

/// The declared type of the sole field of a top-level `class C { <field> }`.
fn sole_field_type(prog: Program<'_>) -> DartType<'_> {
    let class = only_class(prog);
    match class.members().next().unwrap() {
        ClassMember::Field(f) => f.field_type().expect("field should carry a type"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn record_return_function_type_in_typedef() {
    let source = "typedef F = (int, int) Function();";
    let parsed = parse_source(source);
    let aliased = typedef_aliased(parsed.tree.program(source));
    let f = as_function(aliased);
    assert!(!f.is_nullable());
    match f.return_type() {
        Some(DartType::Record(r)) => assert_eq!(r.positional().len(), 2),
        other => panic!("expected record return type, got {other:?}"),
    }
}

#[test]
fn record_return_function_type_as_field_type() {
    let source = "class C { (int, int) Function()? f; }";
    let parsed = parse_source(source);
    let ty = sole_field_type(parsed.tree.program(source));
    let f = as_function(ty);
    assert!(f.is_nullable(), "field function type should be nullable");
    match f.return_type() {
        Some(DartType::Record(r)) => assert_eq!(r.positional().len(), 2),
        other => panic!("expected record return type, got {other:?}"),
    }
}

fn only_function(prog: Program<'_>) -> FunctionDecl<'_> {
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn generic_inline_function_typed_positional_formal() {
    let source = "void f(int cb<T>(T x)) {}";
    let parsed = parse_source(source);
    let params = only_function(parsed.tree.program(source)).params().unwrap();
    let p = params.params().next().unwrap();
    assert_eq!(p.kind(), ParamKind::Positional);
    assert_eq!(p.name().name(), "cb");
    assert!(
        p.function_params().is_some(),
        "expected a function-typed formal"
    );
}

#[test]
fn generic_inline_function_typed_named_formal() {
    let source = "void f({int cb<T>(T x)?}) {}";
    let parsed = parse_source(source);
    let p = only_function(parsed.tree.program(source))
        .params()
        .unwrap()
        .params()
        .next()
        .unwrap();
    assert_eq!(p.kind(), ParamKind::Named { required: false });
    assert_eq!(p.name().name(), "cb");
    assert!(p.function_params().is_some());
}

#[test]
fn generic_inline_function_typed_type_param_return() {
    let source = "void f(T select<T>(List<T> xs)) {}";
    let parsed = parse_source(source);
    let p = only_function(parsed.tree.program(source))
        .params()
        .unwrap()
        .params()
        .next()
        .unwrap();
    assert_eq!(p.kind(), ParamKind::Positional);
    assert_eq!(p.name().name(), "select");
    assert!(p.function_params().is_some());
}

#[test]
fn generic_function_typed_formal_rejected_inside_function_type() {
    // A generic function-typed formal is NOT valid inside a generic function
    // TYPE — Dart rejects it and falcon must keep rejecting it there.
    let falcon_dart_parser::ParseResult { errors, .. } =
        parse("typedef Old = void Function(int cb<T>(T x));");
    assert!(
        !errors.is_empty(),
        "expected rejection inside function type"
    );
}

#[test]
fn test_generic_function_typed_formal_retains_type_params() {
    // `int cb<T>(T x)` must be distinguishable from `int cb(T x)` in the syntax.
    let source = "void f(int cb<T>(T x)) {}";
    let parsed = parse(source);
    assert_eq!(parsed.errors.len(), 0, "{:?}", parsed.errors);
    let prog = parsed.tree.program(source);
    let func = match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    let param = func.params().unwrap().params().next().unwrap();
    assert_eq!(param.kind(), ParamKind::Positional);
    assert!(param.function_params().is_some());
    assert_eq!(param.type_params().len(), 1, "generic <T> must be retained");
    assert_eq!(param.type_params().next().unwrap().name().name(), "T");

    let source2 = "void f(int cb(int x)) {}";
    let parsed2 = parse(source2);
    assert_eq!(parsed2.errors.len(), 0, "{:?}", parsed2.errors);
    let prog2 = parsed2.tree.program(source2);
    let func2 = match prog2.declarations().next().unwrap() {
        TopLevelDecl::Function(f) => f,
        other => panic!("got {other:?}"),
    };
    assert_eq!(
        func2.params().unwrap().params().next().unwrap().kind(),
        ParamKind::Positional
    );
    assert!(
        func2
            .params()
            .unwrap()
            .params()
            .next()
            .unwrap()
            .type_params()
            .is_empty()
    );
}
