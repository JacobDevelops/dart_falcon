use falcon_dart_parser::{parse, parse_for_package};
use falcon_syntax::LanguageVersion;
use falcon_syntax::ast::*;

fn switch_case(source: &str, package_version: Option<LanguageVersion>) -> SwitchCaseKind {
    let (program, errors) = parse_for_package(source, package_version);
    assert!(errors.is_empty(), "source: {source}\nerrors: {errors:?}");
    let TopLevelDecl::Function(function) = &program.declarations[0] else {
        panic!("expected function, got {:?}", program.declarations);
    };
    let Some(FunctionBody::Block(block)) = &function.body else {
        panic!("expected block, got {:?}", function.body);
    };
    let Stmt::Switch(switch) = &block.stmts[0] else {
        panic!("expected switch, got {:?}", block.stmts);
    };
    assert_eq!(switch.cases.len(), 1);
    assert_eq!(switch.cases[0].cases.len(), 1);
    switch.cases[0].cases[0].clone()
}

#[test]
fn file_marker_precedes_package_language_for_switch_grammar() {
    let body = "void f(Object o) { switch (o) { case [1]: break; } }";
    for (prefix, package, legacy) in [
        ("", None, false),
        ("", Some(LanguageVersion::new(2, 19)), true),
        ("// @dart = 2.19\n", Some(LanguageVersion::new(3, 13)), true),
        (
            "/* c */\n// @dart = 2.19\n",
            Some(LanguageVersion::new(3, 13)),
            true,
        ),
        ("// @dart = 3.0\n", Some(LanguageVersion::new(2, 19)), false),
        ("// @dart = 2\n", Some(LanguageVersion::new(3, 13)), false),
        (
            "// @dart = 2.19.0\n",
            Some(LanguageVersion::new(3, 13)),
            false,
        ),
        (
            "// @dart = 2.19 trailing\n",
            Some(LanguageVersion::new(3, 13)),
            false,
        ),
        (
            "// @dart = 2.19.0\n",
            Some(LanguageVersion::new(2, 19)),
            true,
        ),
        (
            "//\t@dart = 2.19\n",
            Some(LanguageVersion::new(3, 13)),
            false,
        ),
    ] {
        let source = format!("{prefix}{body}");
        let case = switch_case(&source, package);
        if legacy {
            assert!(
                matches!(case, SwitchCaseKind::Expr(expr) if matches!(*expr, Expr::List { .. })),
                "{source}"
            );
        } else {
            assert!(
                matches!(case, SwitchCaseKind::Pattern(pattern, None) if matches!(*pattern, Pattern::List(_))),
                "{source}"
            );
        }
    }
}

#[test]
fn legacy_switch_labels_keep_full_expression_ast() {
    let source = "// @dart = 2.19\nvoid f(Object o) { switch (o) {\ncase 1 + 2: break;\ncase true ? 1 : 2: break;\ncase 1 is int: break;\ncase -x: break;\ncase !b: break;\ncase 1 == 2: break;\ncase x ?? 2: break;\ncase 'text'.length: break;\n} }";
    let (program, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = &program.declarations[0] else {
        panic!("expected function");
    };
    let Some(FunctionBody::Block(block)) = &function.body else {
        panic!("expected block");
    };
    let Stmt::Switch(switch) = &block.stmts[0] else {
        panic!("expected switch");
    };
    assert_eq!(switch.cases.len(), 8);
    for (index, group) in switch.cases.iter().enumerate() {
        let SwitchCaseKind::Expr(expr) = &group.cases[0] else {
            panic!("expected expression case {index}, got {:?}", group.cases);
        };
        assert!(
            match index {
                0 | 5 | 6 => matches!(expr.as_ref(), Expr::Binary { .. }),
                1 => matches!(expr.as_ref(), Expr::Conditional { .. }),
                2 => matches!(expr.as_ref(), Expr::Is { .. }),
                3 | 4 => matches!(expr.as_ref(), Expr::Unary { .. }),
                7 => matches!(expr.as_ref(), Expr::Field { field, .. } if field.name == "length"),
                _ => unreachable!(),
            },
            "case {index}: {expr:?}"
        );
    }
}

#[test]
fn primary_constructor_syntax_requires_313() {
    for source in [
        "class C(int x) {}",
        "class C.named(int x) {}",
        "class C { new(); }",
        "class C(int x) { this : assert(x > 0); }",
        "class C;",
        "class C { final int _x; C({required this._x}); }",
    ] {
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 12)));
        assert!(
            errors
                .iter()
                .any(|error| error.message.starts_with("This requires the '")),
            "{source}: {errors:?}"
        );
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 13)));
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
    let (_, errors) = parse_for_package(
        "extension type E(int value) {}",
        Some(LanguageVersion::new(3, 12)),
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn empty_class_feature_gate_reports_the_consumed_semicolon() {
    for source in [
        "class Empty;\n",
        "class Empty; /* next declaration */ class Next {}",
        "// café\nclass Empty;\n",
    ] {
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 9)));
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        assert_eq!(
            errors[0].message,
            "This requires the 'primary-constructors' language feature"
        );
        assert_eq!(errors[0].offset, source.find(';').unwrap(), "{source}");
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 13)));
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn private_named_formal_feature_gate_reports_the_field_name() {
    for source in [
        "class A {\n  final int _x;\n  A({required this._x});\n}",
        "class A { final int _x; A({this._x = 1}); }",
        "// café\nclass A { final int _x; A({required this._x /* field */}); }",
    ] {
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 9)));
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        assert_eq!(
            errors[0].message,
            "This requires the 'private-named-parameters' language feature"
        );
        assert_eq!(errors[0].offset, source.rfind("_x").unwrap(), "{source}");
        let (_, errors) = parse_for_package(source, Some(LanguageVersion::new(3, 13)));
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}
