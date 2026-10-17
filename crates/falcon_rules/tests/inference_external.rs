//! Analyzer-confirmed inference regressions that need external member signatures.
//! Like falcon_analyze's external_libraries tests, these load parsed Dart source
//! from a small local SDK-shaped tree, never an installed SDK or production table.

#[path = "support/inference.rs"]
mod inference;

use falcon_analyze::RuleAnalysis;
use falcon_rules::all_rules;
use inference::assert_lints as assert_external_lints;

#[test]
fn unmodifiable_list_view_inherits_element_type() {
    assert_external_lints(
        "collection-methods-unrelated-type",
        r#"import 'dart:collection';
void inferredReceivers(UnmodifiableListView<int> view) {
  view.contains('x'); /* expect: collection-methods-unrelated-type */
  view.contains(1);
}
"#,
        1,
    );
}

#[test]
fn inherited_sdk_where_chain_keeps_closure_slots() {
    assert_external_lints(
        "null-closures",
        r#"void inheritedSdkChain(List<int> values) {
  values.where((value) => true).firstWhere((value) => true, orElse: null); /* expect: null-closures */
  values.where((value) => true).firstWhere((value) => true, orElse: () => 0);
}
"#,
        1,
    );
}

#[test]
fn nullable_callback_calls_preserve_declared_return_types() {
    assert_external_lints(
        "omit-local-variable-types",
        include_str!("corpus/omit-local-variable-types/nullable_callbacks.dart"),
        10,
    );
}

#[test]
fn inherited_getter_and_callback_parameter_are_boolean() {
    assert_external_lints(
        "no-literal-bool-comparisons",
        r#"extension E on List<int> {
  bool m() => isNotEmpty == true; /* expect: no-literal-bool-comparisons */
}
void closureParameter(List<bool> l) {
  l.any((e) => e == true); /* expect: no-literal-bool-comparisons */
}
"#,
        2,
    );
}

#[test]
fn string_index_and_operator_keep_string_receiver_type() {
    assert_external_lints(
        "noop-primitive-operations",
        r#"void indexed(String s) {
  s[0].toString(); /* expect: noop-primitive-operations */
}
void multiply(String s) {
  (s * 2).toString(); /* expect: noop-primitive-operations */
}
"#,
        2,
    );
}

#[test]
fn inherited_list_getters_preserve_throw_type() {
    assert_external_lints(
        "only-throw-errors",
        r#"void typed(List<Object> errors) {
  throw errors.first; /* expect: only-throw-errors */
}
void inferred() {
  final errors = <Object>[];
  try {
    throw Error();
  } catch (error) {
    errors.add(error);
  }
  throw errors.first; /* expect: only-throw-errors */
}
"#,
        2,
    );
}

#[test]
fn raw_generics_function_joins_extensions_null_shorting_and_overrides() {
    assert_external_lints(
        "omit-local-variable-types",
        include_str!("corpus/omit-local-variable-types/residual_inference.dart"),
        8,
    );
}

#[test]
fn redundant_constant_collections_records_and_types() {
    assert_external_lints(
        "avoid-redundant-argument-values",
        r#"void list({List<int> x = const []}) {}
void record({(int, String) x = (1, 'a')}) {}
void type({Type x = int}) {}
void f() {
  list(x: const <int>[]); /* expect: avoid-redundant-argument-values */
  record(x: (1, 'a')); /* expect: avoid-redundant-argument-values */
  type(x: int); /* expect: avoid-redundant-argument-values */
}
"#,
        3,
    );
}

#[test]
fn callable_inherited_enum_and_annotation_integer_contexts() {
    assert_external_lints(
        "prefer-int-literals",
        r#"class C { double call(double x) => x; }
abstract class I { void m(double x); }
class J implements I { void m(x) {} }
void f(C c, J j) {
  c(1.0); /* expect: prefer-int-literals */
  j.m(1.0); /* expect: prefer-int-literals */
}
enum E {
  a(1.0); /* expect: prefer-int-literals */
  const E(this.value);
  final double value;
}
class A { const A(double x); }
@A(1.0) /* expect: prefer-int-literals */
void annotated() {}
"#,
        4,
    );
}

#[test]
fn null_shorting_in_declaration_initializers_keeps_extension_results() {
    assert_external_lints(
        "omit-local-variable-types",
        r#"class Leaf {}
class Root { Leaf get leaf => Leaf(); }
extension LeafValue on Leaf { int get value => 1; int read() => 1; }
Root? root;
List<Leaf>? list;
final selected = root?.leaf.value;
final called = root?.leaf.read();
final indexed = list?[0].value;
class Holder {
  final selected = root?.leaf.value;
  final called = root?.leaf.read();
  final indexed = list?[0].value;
}
void use(Holder holder) {
  final int? a = selected; /* expect: omit-local-variable-types */
  final int? b = called; /* expect: omit-local-variable-types */
  final int? c = indexed; /* expect: omit-local-variable-types */
  final int? d = holder.selected; /* expect: omit-local-variable-types */
  final int? e = holder.called; /* expect: omit-local-variable-types */
  final int? f = holder.indexed; /* expect: omit-local-variable-types */
}
"#,
        6,
    );
}

#[test]
fn parameter_and_local_annotations_use_constructor_argument_contexts() {
    assert_external_lints(
        "prefer-int-literals",
        r#"class A { const A(double value); }
void top(@A(1.0) int value) {} /* expect: prefer-int-literals */
void nested(void callback(@A(1.0) int value)) {} /* expect: prefer-int-literals */
void deep(void callback(void inner(@A(1.0) int value))) {} /* expect: prefer-int-literals */
void nestedGeneric(void callback<@A(1.0) T>( /* expect: prefer-int-literals */
  @A(1.0) T value)) {} /* expect: prefer-int-literals */
void generic<@A(1.0) T>() {} /* expect: prefer-int-literals */
extension type P(@A(1.0) int value) {} /* expect: prefer-int-literals */
class C {
  C(@A(1.0) int value); /* expect: prefer-int-literals */
  void method(@A(1.0) int value) {} /* expect: prefer-int-literals */
  void generic<@A(1.0) T>() {} /* expect: prefer-int-literals */
  set value(@A(1.0) int value) {} /* expect: prefer-int-literals */
  void locals() {
    @A(1.0) final local = 0; /* expect: prefer-int-literals */
    @A(1.0) void localFunction() {} /* expect: prefer-int-literals */
    void localGeneric<@A(1.0) T>() {} /* expect: prefer-int-literals */
    final genericCallback = <@A(1.0) T>() {}; /* expect: prefer-int-literals */
    for (@A(1.0) var i = 0; i < 1; i++) {} /* expect: prefer-int-literals */
    for (@A(1.0) final value in <int>[]) {} /* expect: prefer-int-literals */
    @A(1.0) final (a, b) = (1, 2); /* expect: prefer-int-literals */
    for (@A(1.0) final (a, b) in <(int, int)>[]) {} /* expect: prefer-int-literals */
    final callback = (@A(1.0) int value) {}; /* expect: prefer-int-literals */
  }
}
"#,
        20,
    );
}

#[test]
fn dependency_frontend_fail_uses_its_declaring_library() {
    // Dart 3.13.5 reports both dependency calls, but not the locally shadowed fail.
    use falcon_analyze::{
        AnalysisEnvironment, AnalyzeContext, ExternalLibraries, ExternalSources, IdentityIndex,
        IdentitySource, LibrarySource, SignatureIndex, TypeIndex,
    };
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use falcon_rules::lint::style::use_test_throws_matchers::UseTestThrowsMatchers;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let app = root.join("app");
    let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
    std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(app.join("lib")).unwrap();
    std::fs::create_dir_all(root.join("test_api/lib/src/frontend")).unwrap();
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::write(
        sdk.join("libraries.json"),
        r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
    )
    .unwrap();
    std::fs::write(
        sdk.join("core/core.dart"),
        include_str!("support/sdk/lib/core/core.dart"),
    )
    .unwrap();
    std::fs::write(
        app.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"flutterRoot":"../../flutter","packages":[
            {"name":"app","rootUri":"../","packageUri":"lib/"},
            {"name":"test_api","rootUri":"../../test_api","packageUri":"lib/"}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("test_api/lib/src/frontend/expect.dart"),
        "Never fail(String message) => throw message;",
    )
    .unwrap();
    let source = r#"import 'package:test_api/src/frontend/expect.dart';
import 'package:test_api/src/frontend/expect.dart' as expectations;
void unprefixed() {
  try { fail('no throw'); /* expect: use-test-throws-matchers */
  } catch (_) {}
}
void prefixed() {
  try { expectations.fail('no throw'); /* expect: use-test-throws-matchers */
  } catch (_) {}
}
void shadowed() {
  void fail(String message) {}
  try { fail('no throw'); } catch (_) {}
}
"#;
    let path = app.join("lib/main.dart");
    std::fs::write(&path, source).unwrap();
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |config| {
        let root = config?.flutter_root.as_ref()?;
        falcon_analyze::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
    });
    let external = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), program, source)],
        |candidate| candidate == path,
    );
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &external,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&external, &identities);
    let signatures = SignatureIndex::build(
        &[(path.clone(), parsed.clone())],
        Some(&external),
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_environment(Some(&environment))
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    let diagnostics = UseTestThrowsMatchers.analyze(program, &context);
    inference::assert_diagnostics(source, "use-test-throws-matchers", &diagnostics, 2);
}

#[test]
fn public_docs_internal_requires_meta_identity() {
    // Dart 3.13.5 reports the fake annotation's declarations, not meta.internal.
    assert_external_lints(
        "public-member-api-docs",
        r#"import 'package:meta/meta.dart' as meta;
/// A locally declared annotation, not package:meta's marker.
const internal = 0;
@internal
class Fake { /* expect: public-member-api-docs */
  Fake(); /* expect: public-member-api-docs */
}
@meta.internal
class Hidden {
  Hidden();
}
@internal
void fakeFunction() {} /* expect: public-member-api-docs */
@meta.internal
void hiddenFunction() {}
@internal
final fakeValue = 1; /* expect: public-member-api-docs */
@meta.internal
final hiddenValue = 1;
"#,
        4,
    );
}

#[test]
fn generic_member_canonical_elements_retain_substituted_occurrences() {
    assert_external_lints(
        "join-return-with-assignment",
        include_str!("corpus/join-return-with-assignment/generic_members.dart"),
        1,
    );
}

#[test]
fn async_closure_future_or_return_context() {
    assert_external_lints(
        "unnecessary-await-in-return",
        include_str!("corpus/unnecessary-await-in-return/bad.dart"),
        15,
    );
}

#[test]
fn extension_static_member_lexical_lookup() {
    assert_external_lints(
        "unnecessary-await-in-return",
        include_str!("corpus/unnecessary-await-in-return/extension_statics.dart"),
        1,
    );
}

#[test]
fn generic_callbacks_keep_downward_parameter_schemas() {
    assert_external_lints(
        "cast-nullable-to-non-nullable",
        include_str!("corpus/cast-nullable-to-non-nullable/contextual_callbacks.dart"),
        6,
    );
}

#[test]
fn conditional_generic_method_tearoffs_keep_callback_contracts() {
    assert_external_lints(
        "unnecessary-await-in-return",
        include_str!("corpus/unnecessary-await-in-return/generic_tearoff.dart"),
        1,
    );
}
