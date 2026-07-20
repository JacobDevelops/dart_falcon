//! Minimal-repro tests for inline function-type parsing gaps closed in the
//! `types` group:
//!   (a) a bare special type (`dynamic`, `void`, `Never`) as the return type of
//!       an inline function type — `dynamic Function(String)`.
//!   (b) generic inline function types with their own type params —
//!       `void Function<T>(T x)`, `void Function<T extends num>(T)`.
//!   (c) function-typed fields inside object patterns ride on (a)/(b).
//!
//! Each test asserts ZERO parse errors and the relevant AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse a single top-level `typedef` and return its aliased type.
fn typedef_aliased(src: &str) -> DartType {
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::TypeAlias(t) => t.aliased.clone(),
        other => panic!("expected typedef, got {other:?}"),
    }
}

fn as_function(ty: &DartType) -> &FunctionType {
    match ty {
        DartType::Function(f) => f,
        other => panic!("expected function type, got {other:?}"),
    }
}

/// The type of the sole positional formal of a top-level `void f(<param>) {}`.
fn sole_param_type(param: &str) -> DartType {
    let (prog, errors) = parse(&format!("void f({param}) {{}}"));
    assert!(errors.is_empty(), "errors: {errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    func.params.positional[0]
        .param_type
        .clone()
        .expect("param should carry a type")
}

// ── (a) special type as function-type return ──────────────────────────────────

#[test]
fn dynamic_function_return_in_typedef() {
    let aliased = typedef_aliased("typedef F = dynamic Function(int);");
    let f = as_function(&aliased);
    assert!(
        matches!(f.return_type.as_deref(), Some(DartType::Dynamic { .. })),
        "return type should be dynamic, got {:?}",
        f.return_type
    );
    assert_eq!(f.params.len(), 1);
    assert!(matches!(f.params[0].param_type, DartType::Named(_)));
}

#[test]
fn dynamic_function_return_as_param() {
    let ty = sole_param_type("dynamic Function(String) cb");
    let f = as_function(&ty);
    assert!(matches!(
        f.return_type.as_deref(),
        Some(DartType::Dynamic { .. })
    ));
    assert_eq!(f.params.len(), 1);
}

#[test]
fn void_function_return_in_typedef() {
    let aliased = typedef_aliased("typedef F = void Function(int);");
    let f = as_function(&aliased);
    assert!(matches!(
        f.return_type.as_deref(),
        Some(DartType::Void { .. })
    ));
}

#[test]
fn never_function_return_in_typedef() {
    let aliased = typedef_aliased("typedef F = Never Function(int);");
    let f = as_function(&aliased);
    // `Never` is a contextual name, so it surfaces as a named return type.
    match f.return_type.as_deref() {
        Some(DartType::Named(n)) => assert_eq!(n.segments[0].name, "Never"),
        other => panic!("expected named `Never` return, got {other:?}"),
    }
}

// ── (b) generic inline function types ─────────────────────────────────────────

#[test]
fn generic_function_type_in_typedef() {
    let aliased = typedef_aliased("typedef F = void Function<T>(T x);");
    let f = as_function(&aliased);
    assert!(matches!(
        f.return_type.as_deref(),
        Some(DartType::Void { .. })
    ));
    assert_eq!(f.type_params.len(), 1);
    assert_eq!(f.type_params[0].name.name, "T");
    assert!(f.type_params[0].bound.is_none());
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name.as_ref().unwrap().name, "x");
}

#[test]
fn generic_function_type_bounded_as_param() {
    let ty = sole_param_type("void Function<T extends num>(T) cb");
    let f = as_function(&ty);
    assert_eq!(f.type_params.len(), 1);
    assert_eq!(f.type_params[0].name.name, "T");
    match &f.type_params[0].bound {
        Some(DartType::Named(n)) => assert_eq!(n.segments[0].name, "num"),
        other => panic!("expected `num` bound, got {other:?}"),
    }
    assert_eq!(f.params.len(), 1);
}

#[test]
fn bare_generic_function_type_in_typedef() {
    let aliased = typedef_aliased("typedef F = Function<T>(T x);");
    let f = as_function(&aliased);
    assert!(f.return_type.is_none());
    assert_eq!(f.type_params.len(), 1);
    assert_eq!(f.type_params[0].name.name, "T");
}

// ── (c) function-typed fields in object patterns ──────────────────────────────

/// Extract the pattern of the first `case` label of a `switch` in a function body.
fn first_case_pattern(src: &str) -> Pattern {
    let wrapped = format!("void f() {{ switch (x) {{ {src} }} }}");
    let (prog, errors) = parse(&wrapped);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let block = match func.body.as_ref().unwrap() {
        FunctionBody::Block(b) => b,
        other => panic!("expected block, got {other:?}"),
    };
    let sw = block
        .stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Switch(sw) => Some(sw),
            _ => None,
        })
        .expect("expected a switch statement");
    match &sw.cases[0].cases[0] {
        SwitchCaseKind::Pattern(p, _) => (**p).clone(),
        other => panic!("expected a pattern case, got {other:?}"),
    }
}

#[test]
fn object_pattern_dynamic_function_field() {
    let pat =
        first_case_pattern("case Typedef(:dynamic Function(Invocation) noSuchMethod): break;");
    let obj = match &pat {
        Pattern::Object(o) => o,
        other => panic!("expected object pattern, got {other:?}"),
    };
    assert_eq!(obj.fields.len(), 1);
    let field = &obj.fields[0];
    assert_eq!(field.name.name, "noSuchMethod");
    match field.pattern.as_ref().expect("field binds a pattern") {
        Pattern::Variable { type_, name, .. } => {
            assert_eq!(name.name, "noSuchMethod");
            let f = as_function(type_.as_ref().expect("variable is typed"));
            assert!(matches!(
                f.return_type.as_deref(),
                Some(DartType::Dynamic { .. })
            ));
        }
        other => panic!("expected typed variable pattern, got {other:?}"),
    }
}

#[test]
fn object_pattern_generic_function_field() {
    let pat = first_case_pattern("case Foo(:void Function<T>(T) cb): break;");
    let obj = match &pat {
        Pattern::Object(o) => o,
        other => panic!("expected object pattern, got {other:?}"),
    };
    match obj.fields[0]
        .pattern
        .as_ref()
        .expect("field binds a pattern")
    {
        Pattern::Variable { type_, .. } => {
            let f = as_function(type_.as_ref().expect("variable is typed"));
            assert_eq!(f.type_params.len(), 1);
            assert_eq!(f.type_params[0].name.name, "T");
        }
        other => panic!("expected typed variable pattern, got {other:?}"),
    }
}

// ── Corpus-found function-type gaps: record return types ──────────────────

/// The declared type of the sole field of a top-level `class C { <field> }`.
fn sole_field_type(field: &str) -> DartType {
    let (prog, errors) = parse(&format!("class C {{ {field} }}"));
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = match &prog.declarations[0] {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    };
    match &class.members[0] {
        ClassMember::Field(f) => f.field_type.clone().expect("field should carry a type"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn record_return_function_type_in_typedef() {
    let aliased = typedef_aliased("typedef F = (int, int) Function();");
    let f = as_function(&aliased);
    assert!(!f.is_nullable);
    match f.return_type.as_deref() {
        Some(DartType::Record(r)) => assert_eq!(r.positional.len(), 2),
        other => panic!("expected record return type, got {other:?}"),
    }
}

#[test]
fn record_return_function_type_as_field_type() {
    let ty = sole_field_type("(int, int) Function()? f;");
    let f = as_function(&ty);
    assert!(f.is_nullable, "field function type should be nullable");
    match f.return_type.as_deref() {
        Some(DartType::Record(r)) => assert_eq!(r.positional.len(), 2),
        other => panic!("expected record return type, got {other:?}"),
    }
}

// ── Generic old-style (inline) function-typed formal parameters ────────────────

fn only_function(prog: &Program) -> &FunctionDecl {
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn generic_inline_function_typed_positional_formal() {
    let (prog, errors) = parse("void f(int cb<T>(T x)) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let params = &only_function(&prog).params;
    let p = &params.positional[0];
    assert_eq!(p.name.name, "cb");
    assert!(p.function_params.is_some(), "expected a function-typed formal");
}

#[test]
fn generic_inline_function_typed_named_formal() {
    let (prog, errors) = parse("void f({int cb<T>(T x)?}) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let p = &only_function(&prog).params.named[0];
    assert_eq!(p.name.name, "cb");
    assert!(p.function_params.is_some());
}

#[test]
fn generic_inline_function_typed_type_param_return() {
    let (prog, errors) = parse("void f(T select<T>(List<T> xs)) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let p = &only_function(&prog).params.positional[0];
    assert_eq!(p.name.name, "select");
    assert!(p.function_params.is_some());
}

#[test]
fn generic_function_typed_formal_rejected_inside_function_type() {
    // A generic function-typed formal is NOT valid inside a generic function
    // TYPE — Dart rejects it and falcon must keep rejecting it there.
    let (_prog, errors) = parse("typedef Old = void Function(int cb<T>(T x));");
    assert!(!errors.is_empty(), "expected rejection inside function type");
}
