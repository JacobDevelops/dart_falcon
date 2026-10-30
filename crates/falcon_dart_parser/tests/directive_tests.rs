//! Regression tests for the `directives` group: metadata before directives
//! (item a) and contextual keywords used as identifiers in name positions
//! (item b). Every fixed construct must parse with zero errors and yield the
//! expected CST view/token shape; role-keeping cases guard against regressions.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

#[path = "common/only_class.rs"]
mod common_class;
#[path = "common/errs.rs"]
mod common_errs;
use common_class::only_class;
use common_errs::errs;

#[test]
fn test_metadata_before_export() {
    let source = "@Deprecated('x') export 'p.dart' show B;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.exports().len(), 1);
    let e = prog.exports().next().unwrap();
    assert_eq!(
        e.annotations().len(),
        1,
        "annotation not attached to export"
    );
    assert_eq!(
        e.annotations()
            .next()
            .unwrap()
            .name()
            .last()
            .unwrap()
            .name(),
        "Deprecated"
    );
    assert_eq!(e.uri().value(), "p.dart");
    match e.combinators().next().unwrap() {
        ImportCombinator::Show(names) => assert_eq!(names.names().next().unwrap().name(), "B"),
        other @ ImportCombinator::Hide(_) => panic!("expected show combinator, got {other:?}"),
    }
    // No stray top-level error node from the leading `@`.
    assert!(
        prog.declarations().is_empty(),
        "unexpected decls: {:?}",
        prog.declarations()
    );
}

#[test]
fn test_metadata_before_import() {
    let source = "@A import 'a.dart';";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports().len(), 1);
    let i = prog.imports().next().unwrap();
    assert_eq!(
        i.annotations().len(),
        1,
        "annotation not attached to import"
    );
    assert_eq!(
        i.annotations()
            .next()
            .unwrap()
            .name()
            .last()
            .unwrap()
            .name(),
        "A"
    );
    assert_eq!(i.uri().value(), "a.dart");
    assert!(prog.declarations().is_empty());
}

#[test]
fn test_metadata_before_part() {
    let source = "@A part 'p.dart';";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.part_directives().len(), 1);
    let p = prog.part_directives().next().unwrap();
    assert_eq!(p.annotations().len(), 1, "annotation not attached to part");
    assert_eq!(
        p.annotations()
            .next()
            .unwrap()
            .name()
            .last()
            .unwrap()
            .name(),
        "A"
    );
    assert_eq!(p.uri().value(), "p.dart");
}

#[test]
fn test_metadata_before_library_dotted_name() {
    let source = "@A library a.b.c;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let lib = prog.library_directive().expect("library directive");
    assert_eq!(
        lib.annotations().len(),
        1,
        "annotation not attached to library"
    );
    assert_eq!(
        lib.annotations()
            .next()
            .unwrap()
            .name()
            .last()
            .unwrap()
            .name(),
        "A"
    );
    let dotted: Vec<_> = lib
        .name()
        .map(falcon_syntax::cst::Identifier::name)
        .collect();
    assert_eq!(dotted, vec!["a", "b", "c"], "dotted library name lost");
}

#[test]
fn test_metadata_with_args_before_part_of() {
    // Annotation carrying an argument list must be skipped when routing `part of`.
    let source = "@A('x') part of foo.bar;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let po = prog.part_of_directive().expect("part-of directive");
    assert_eq!(
        po.annotations().len(),
        1,
        "annotation not attached to part-of"
    );
    let dotted: Vec<_> = po
        .name()
        .map(falcon_syntax::cst::Identifier::name)
        .collect();
    assert_eq!(dotted, vec!["foo", "bar"]);
}

#[test]
fn test_getter_named_on() {
    let source = "class C { Reference? get on; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match only_class(prog).members().next().unwrap() {
        ClassMember::Getter(g) => {
            assert_eq!(g.name().name(), "on");
            assert!(g.return_type().is_some(), "getter return type lost");
        }
        other => panic!("expected getter, got {other:?}"),
    }
}

#[test]
fn test_method_named_show() {
    let source = "class C { void show() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => assert_eq!(m.name().name(), "show"),
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_field_named_hide() {
    let source = "class C { int hide = 0; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match only_class(prog).members().next().unwrap() {
        ClassMember::Field(f) => assert_eq!(f.declarators().next().unwrap().name().name(), "hide"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn test_param_named_when() {
    let source = "void f(int when) {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(fun) => {
            let param = fun.params().unwrap().params().next().unwrap();
            assert_eq!(param.kind(), ParamKind::Positional);
            assert_eq!(param.name().name(), "when");
        }
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_local_var_named_of() {
    let source = "void g() { var of = 1; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Function(_) => {}
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_getter_named_override() {
    // `override` is a builtin identifier; it must be usable as a getter name too.
    let source = "class C { bool get override => true; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match only_class(prog).members().next().unwrap() {
        ClassMember::Getter(g) => assert_eq!(g.name().name(), "override"),
        other => panic!("expected getter, got {other:?}"),
    }
}

#[test]
fn test_mixin_on_clause_still_parses() {
    let source = "mixin M on A {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Mixin(m) => {
            assert_eq!(m.on_clause().len(), 1, "on-clause lost");
        }
        other => panic!("expected mixin, got {other:?}"),
    }
}

#[test]
fn test_import_show_hide_still_parses() {
    let source = "import 'a.dart' show A hide B;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let combos = prog.imports().next().unwrap().combinators();
    assert!(matches!(
        combos.clone().next().unwrap(),
        ImportCombinator::Show(..)
    ));
    assert!(matches!(
        combos.clone().nth(1).unwrap(),
        ImportCombinator::Hide(..)
    ));
}

#[test]
fn test_part_of_still_parses() {
    let source = "part of foo.bar;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let po = prog.part_of_directive().expect("part-of directive");
    let dotted: Vec<_> = po
        .name()
        .map(falcon_syntax::cst::Identifier::name)
        .collect();
    assert_eq!(dotted, vec!["foo", "bar"]);
}

#[test]
fn test_async_body_marker_still_parses() {
    let source = "class C { foo() async {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "foo");
            assert_eq!(m.async_marker(), AsyncMarker::Async, "async marker lost");
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn import_adjacent_string_uri() {
    assert_eq!(errs("import 'package:foo' '/bar.dart';"), 0);
}

#[test]
fn export_adjacent_string_uri() {
    assert_eq!(errs("export 'a' '/b.dart';"), 0);
}
