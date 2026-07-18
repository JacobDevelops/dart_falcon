//! Regression tests for the `directives` group: metadata before directives
//! (item a) and contextual keywords used as identifiers in name positions
//! (item b). Every fixed construct must parse with zero errors and yield the
//! expected AST/token shape; role-keeping cases guard against regressions.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse whole-program `src` and return its parse-error count.
fn errs(src: &str) -> usize {
    parse(src).1.len()
}

fn only_class(prog: &Program) -> &ClassDecl {
    match &prog.declarations[0] {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    }
}

// ── Item (a): metadata before directives ──────────────────────────────────────

#[test]
fn test_metadata_before_export() {
    let (prog, errors) = parse("@Deprecated('x') export 'p.dart' show B;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.exports.len(), 1);
    let e = &prog.exports[0];
    assert_eq!(e.annotations.len(), 1, "annotation not attached to export");
    assert_eq!(e.annotations[0].name.last().unwrap().name, "Deprecated");
    assert_eq!(e.uri.value, "p.dart");
    match &e.combinators[0] {
        ImportCombinator::Show(names, _) => assert_eq!(names[0].name, "B"),
        other => panic!("expected show combinator, got {other:?}"),
    }
    // No stray top-level error node from the leading `@`.
    assert!(
        prog.declarations.is_empty(),
        "unexpected decls: {:?}",
        prog.declarations
    );
}

#[test]
fn test_metadata_before_import() {
    let (prog, errors) = parse("@A import 'a.dart';");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports.len(), 1);
    let i = &prog.imports[0];
    assert_eq!(i.annotations.len(), 1, "annotation not attached to import");
    assert_eq!(i.annotations[0].name.last().unwrap().name, "A");
    assert_eq!(i.uri.value, "a.dart");
    assert!(prog.declarations.is_empty());
}

#[test]
fn test_metadata_before_part() {
    let (prog, errors) = parse("@A part 'p.dart';");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.part_directives.len(), 1);
    let p = &prog.part_directives[0];
    assert_eq!(p.annotations.len(), 1, "annotation not attached to part");
    assert_eq!(p.annotations[0].name.last().unwrap().name, "A");
    assert_eq!(p.uri.value, "p.dart");
}

#[test]
fn test_metadata_before_library_dotted_name() {
    let (prog, errors) = parse("@A library a.b.c;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let lib = prog.library_directive.as_ref().expect("library directive");
    assert_eq!(
        lib.annotations.len(),
        1,
        "annotation not attached to library"
    );
    assert_eq!(lib.annotations[0].name.last().unwrap().name, "A");
    let dotted: Vec<_> = lib.name.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(dotted, vec!["a", "b", "c"], "dotted library name lost");
}

#[test]
fn test_metadata_with_args_before_part_of() {
    // Annotation carrying an argument list must be skipped when routing `part of`.
    let (prog, errors) = parse("@A('x') part of foo.bar;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let po = prog.part_of_directive.as_ref().expect("part-of directive");
    assert_eq!(
        po.annotations.len(),
        1,
        "annotation not attached to part-of"
    );
    let dotted: Vec<_> = po.name.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(dotted, vec!["foo", "bar"]);
}

// ── Item (b): contextual keywords as identifiers in name positions ────────────

#[test]
fn test_getter_named_on() {
    let (prog, errors) = parse("class C { Reference? get on; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Getter(g) => {
            assert_eq!(g.name.name, "on");
            assert!(g.return_type.is_some(), "getter return type lost");
        }
        other => panic!("expected getter, got {other:?}"),
    }
}

#[test]
fn test_method_named_show() {
    let (prog, errors) = parse("class C { void show() {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => assert_eq!(m.name.name, "show"),
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_field_named_hide() {
    let (prog, errors) = parse("class C { int hide = 0; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Field(f) => assert_eq!(f.declarators[0].name.name, "hide"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn test_param_named_when() {
    let (prog, errors) = parse("void f(int when) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(fun) => {
            assert_eq!(fun.params.positional[0].name.name, "when");
        }
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_local_var_named_of() {
    let (prog, errors) = parse("void g() { var of = 1; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(_) => {}
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_getter_named_override() {
    // `override` is a builtin identifier; it must be usable as a getter name too.
    let (prog, errors) = parse("class C { bool get override => true; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Getter(g) => assert_eq!(g.name.name, "override"),
        other => panic!("expected getter, got {other:?}"),
    }
}

// ── Item (b): keyword roles must keep working ─────────────────────────────────

#[test]
fn test_mixin_on_clause_still_parses() {
    let (prog, errors) = parse("mixin M on A {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Mixin(m) => {
            assert_eq!(m.on_clause.len(), 1, "on-clause lost");
        }
        other => panic!("expected mixin, got {other:?}"),
    }
}

#[test]
fn test_import_show_hide_still_parses() {
    let (prog, errors) = parse("import 'a.dart' show A hide B;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let combos = &prog.imports[0].combinators;
    assert!(matches!(combos[0], ImportCombinator::Show(..)));
    assert!(matches!(combos[1], ImportCombinator::Hide(..)));
}

#[test]
fn test_part_of_still_parses() {
    let (prog, errors) = parse("part of foo.bar;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let po = prog.part_of_directive.as_ref().expect("part-of directive");
    let dotted: Vec<_> = po.name.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(dotted, vec!["foo", "bar"]);
}

#[test]
fn test_async_body_marker_still_parses() {
    let (prog, errors) = parse("class C { foo() async {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "foo");
            assert!(m.is_async, "async marker lost");
        }
        other => panic!("expected method, got {other:?}"),
    }
}

// ── Corpus-found directive gaps: adjacent-string URIs ────────────────────

#[test]
fn import_adjacent_string_uri() {
    assert_eq!(errs("import 'package:foo' '/bar.dart';"), 0);
}

#[test]
fn export_adjacent_string_uri() {
    assert_eq!(errs("export 'a' '/b.dart';"), 0);
}
