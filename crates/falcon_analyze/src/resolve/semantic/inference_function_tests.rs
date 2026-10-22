use super::super::tests::with_model;
use super::*;
use std::cell::Cell;

fn assert_nested_assignment_closures_inferred_once(conditional: bool) {
    let depth = 12;
    let mut source = String::from("dynamic f() => () { dynamic b; ");
    for _ in 0..depth {
        if conditional {
            source.push_str("if (true) { ");
        }
        source.push_str("b = () { ");
    }
    source.push_str("probe;");
    for _ in 0..depth {
        source.push_str("};");
        if conditional {
            source.push('}');
        }
    }
    source.push_str("};");
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Arrow(body)) = function.body() else {
            panic!("expected arrow body");
        };
        let probes = Cell::new(0);
        let inferred = TypeEnvironment::new().infer_with_signatures_using_lookup(
            body.expr(),
            model,
            &signatures,
            &TypeParameterScope::default(),
            &|name| {
                (name == "probe").then(|| {
                    probes.set(probes.get() + 1);
                    ResolvedType::DYNAMIC
                })
            },
        );
        let ResolvedType {
            kind: ResolvedTypeKind::Function { return_type, .. },
        } = inferred
        else {
            panic!("expected closure type");
        };
        assert_eq!(*return_type, ResolvedType::NULL);
        assert_eq!(
            probes.get(),
            1,
            "nested bodies must not be reinferred for completion",
        );
    });
}

#[test]
fn nested_assignment_closures_are_inferred_once() {
    assert_nested_assignment_closures_inferred_once(false);
}

#[test]
fn conditionally_nested_assignment_closures_are_inferred_once() {
    assert_nested_assignment_closures_inferred_once(true);
}

#[test]
fn closure_inference_does_not_cache_flow_or_expected_types() {
    with_model(
        "dynamic f() => (value) { return (captured, value); }; \
         (int, int) Function(int) first() => throw 0; \
         (String, String) Function(String) second() => throw 0;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let mut declarations = program.declarations();
            let TopLevelDecl::Function(function) = declarations.next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!("expected arrow body");
            };
            let mut environment = TypeEnvironment::new();
            for declaration in declarations {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function");
                };
                let expected = model.resolve_type(function.return_type().unwrap());
                let ResolvedType {
                    kind: ResolvedTypeKind::Function { positional, .. },
                } = &expected
                else {
                    panic!("expected function type");
                };
                environment.declare("captured".to_owned(), positional[0].clone());
                assert_eq!(
                    environment.infer_with_context(
                        body.expr(),
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                        Some(&expected),
                        &|_| None,
                    ),
                    expected,
                );
            }
        },
    );
}

#[test]
fn completion_preserves_never_calls_assignments_and_fallthrough() {
    with_model(
        "Never fail() => throw 0; \
         int Function(bool) call() => (bool flag) { if (flag) return 1; fail(); }; \
         int Function(bool) assign() => (bool flag) { dynamic b; if (flag) return 1; b = fail(); }; \
         int Function(bool) blockScoped() => (bool c) { \
           if (c) return 1; { final Never Function() h = fail; h(); } \
         }; \
         int Function(bool) demoted() => (bool c) { \
           if (c) return 1; Object o = fail; \
           if (o is! Never Function()) return 2; o(); o = 3; \
         }; \
         int? Function(bool) partial() => (bool flag) { if (flag) return 1; 0; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations().skip(1) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body.expr(),
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn long_binary_chains_in_closures_and_conditions_use_normal_stack() {
    let chain = " + 0".repeat(100_000);
    for source in [
        format!("int Function() f() => () {{ int? x; (x = 1){chain}; return x; }};"),
        format!("int Function() f() => () {{ int? x; if (x!{chain} > 0) {{}} return x; }};"),
    ] {
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!("expected arrow body");
            };
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    body.expr(),
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                ),
                model.resolve_type(function.return_type().unwrap()),
            );
        });
    }
}

#[test]
fn recorded_never_branch_preserves_surviving_assignment_promotion() {
    with_model(
        "Never failNever() => throw 0; \
         int Function(bool) f() => (bool flag) { \
           int? x; if (flag) { failNever(); } else { x = 1; } return x; \
         };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!("expected arrow body");
            };
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    body.expr(),
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                ),
                model.resolve_type(function.return_type().unwrap()),
            );
        },
    );
}

#[test]
fn closure_flow_joins_keep_nested_optional_writes_nullable() {
    with_model(
        "int? Function(bool) loop() => (bool flag) { int? value; \
         while (flag) { int inner = 1; if (flag) value = inner; break; } return value; }; \
         int? Function(bool) caught() => (bool flag) { int? value; \
         try { if (flag) { int inner = 1; value = inner; } else { throw 0; } } \
         catch (_) { int inner = 2; value = inner; } finally { int inner = 3; } return value; }; \
         int? Function(bool) switched() => (bool flag) { int? value; \
         switch (flag) { case true: { int inner = 1; value = inner; break; } \
         case false: break; } return value; }; \
         int? Function(bool) conjunction() => (bool flag) { int? value; \
         flag && ((value = 1) > 0); return value; }; \
         int? Function(bool) disjunction() => (bool flag) { int? value; \
         flag || ((value = 1) > 0); return value; }; \
         int? Function(bool?) coalesced() => (bool? flag) { int? value; \
         flag ?? ((value = 1) > 0); return value; }; \
         int Function(bool) leftAssigned() => (bool flag) { int? value; \
         ((value = 1) > 0) && flag; return value; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body.expr(),
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}
