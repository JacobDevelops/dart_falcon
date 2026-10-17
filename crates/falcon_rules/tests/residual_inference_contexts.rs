//! SDK-free regressions reduced from the residual omit-local and closure rows.
//! The analyzer expectations were checked with isolated analyzer probes; no SDK
//! member names are encoded in the resolver to support these cases.
//! Joined callable signatures retain typedef annotations only when both
//! operands preserve the same alias instance, including nullable joins.

#[path = "support/inference.rs"]
mod inference;

use std::path::Path;

use falcon_analyze::{
    AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, RuleAnalysis, SignatureIndex,
    TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::all_rules;

use inference::{assert_diagnostics, assert_lints};

#[test]
fn generic_callback_results_ignore_unknown_downward_schema_slots() {
    assert_lints(
        "unrelated-type-equality-checks",
        r#"
T pick<T>(T left, T right) => left;
T identity<T>(T value) => value;
bool entry() {
  return [1].map((value) => MapEntry(value, 'text')).first.value == 1; /* expect: unrelated-type-equality-checks */
}
bool invocation() {
  return [1].map((value) => pick(value, 2)).first == 'text'; /* expect: unrelated-type-equality-checks */
}
bool tearoff() {
  return [1].map(identity).first == 'text'; /* expect: unrelated-type-equality-checks */
}
"#,
        3,
    );
}

#[test]
fn spread_only_maps_preserve_value_types_for_equality_checks() {
    assert_lints(
        "unrelated-type-equality-checks",
        r#"
bool different(Map<String, String> input) {
  return {...input}.values.first == 1; /* expect: unrelated-type-equality-checks */
}
bool compatible(Map<String, String> input) {
  return {...input}.values.first == 'value';
}
"#,
        1,
    );
}

#[test]
fn omitted_future_value_argument_infers_dynamic_through_await() {
    assert_lints(
        "avoid-dynamic-calls",
        r#"
Future<void> omitted() async {
  (await Future.value()).add(1); /* expect: avoid-dynamic-calls */
}
Future<void> explicitDynamic() async {
  (await Future<dynamic>.value()).add(1); /* expect: avoid-dynamic-calls */
}
Future<void> explicitList() async {
  (await Future<List<int>>.value()).add(1);
}
Future<void> inferredList() async {
  (await Future.value(<int>[])).add(1);
}
"#,
        2,
    );
}

#[test]
fn callable_objects_receive_function_context_for_local_type_omission() {
    assert_lints(
        "omit-local-variable-types",
        r#"
class Callable { void call() {} }
void use(Callable value, dynamic dynamicValue) {
  void Function() callback = value; /* expect: omit-local-variable-types */
  Callable ordinary = value; /* expect: omit-local-variable-types */
  void Function() dynamicCallback = dynamicValue;
  callback();
  ordinary();
  dynamicCallback();
}
"#,
        2,
    );
}

#[test]
fn historical_callable_contexts_preserve_exact_omission_spans() {
    type ExpectedSpan<'a> = (usize, usize, &'a str);
    let cases: &[(&str, &[ExpectedSpan<'_>])] = &[
        (
            r#"class C { void call() {} int x = 0; }
void f(C c, C c2, C? n, bool b) {
  void Function() v1 = b ? c : c2;
  void Function() v2 = c..x = 1;
  void Function() v3 = n ?? c;
  print([v1, v2, v3]);
}
"#,
            &[
                (3, 3, "void Function()"),
                (4, 3, "void Function()"),
                (5, 3, "void Function()"),
            ],
        ),
        (
            r#"import 'dart:async';
class C { void call() {} }
extension type XC(C c) implements C {}
void takeFO(FutureOr<void Function()> f) {}
void f(C c, XC xc) {
  void Function() a = xc;
  takeFO(c);
  var l = <void Function()>[...[c]];
  (void Function(), int) r = (c, 1);
  ({void Function() f}) r2 = (f: c);
  print([a, l, r, r2]);
}
Future<void Function()> fut(C c) async => c;
Stream<void Function()> st(C c) async* { yield c; }
Iterable<void Function()> it(C c) sync* { yield c; }
void k<T extends C>(T t) {
  void Function() q = t;
  print(q);
}
class Gen { T call<T>(T t) => t; }
void g(Gen g) {
  int Function(int) h2 = g<int>;
  var h3 = g<int>;
  print([h2, h3]);
}
"#,
            &[
                (6, 3, "void Function()"),
                (9, 3, "(void Function(), int)"),
                (10, 3, "({void Function() f})"),
                (17, 3, "void Function()"),
                (22, 3, "int Function(int)"),
            ],
        ),
        (
            // The analyzer rejects assigning nullable C directly to Function?,
            // and does not report omission for either nullable function local.
            r#"class C { void call() {} int x = 0; }
class Gen { T call<T>(T t) => t; }
void f(C c, C c2, C? n, bool b, Gen g) {
  void Function() conditional = b ? c : c2;
  void Function() cascade = c..x = 1;
  void Function() coalesce = n ?? c;
  void Function()? nullable = n;
  void Function()? nonnullable = c;
  int Function(int) generic = g<int>;
  int Function(int) explicitCall = g.call<int>;
  Object ordinary = n ?? c;
  C retained = n ?? c;
  print([conditional, cascade, coalesce, nullable, nonnullable, generic, explicitCall, ordinary, retained]);
}
"#,
            &[
                (4, 3, "void Function()"),
                (5, 3, "void Function()"),
                (6, 3, "void Function()"),
                (9, 3, "int Function(int)"),
                (10, 3, "int Function(int)"),
                (12, 3, "C"),
            ],
        ),
    ];
    let rules = all_rules();
    let rule = &rules
        .iter()
        .find(|(name, _)| *name == "omit-local-variable-types")
        .unwrap()
        .1;
    for (source, expected) in cases {
        let diagnostics = inference::analyze(
            rule.as_ref(),
            Path::new("historical_callable_contexts.dart"),
            source,
            &FalconConfig::default(),
        );
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                let prefix = &source[..diagnostic.span.start];
                (
                    prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
                    prefix.rsplit('\n').next().unwrap().len() + 1,
                    &source[diagnostic.span.start..diagnostic.span.end],
                )
            })
            .collect();
        assert_eq!(actual.as_slice(), *expected, "{source}");
    }
}

#[test]
fn late_const_declaration_span_starts_at_late_keyword() {
    let source = r#"class A {
  static late final a35 = 1;
  static final /* late */ commented = 2;
}
late final top = 1;
void locals() {
  late final local = 1;
  print(local);
}
"#;
    let rules = all_rules();
    let rule = &rules
        .iter()
        .find(|(name, _)| *name == "prefer-const-declarations")
        .unwrap()
        .1;
    let diagnostics = inference::analyze(
        rule.as_ref(),
        Path::new("historical_const_span.dart"),
        source,
        &FalconConfig::default(),
    );
    let expected = [
        "late final a35 = 1",
        "final /* late */ commented = 2",
        "late final top = 1",
        "late final local = 1",
    ];
    assert_eq!(diagnostics.len(), expected.len());
    for (diagnostic, expected) in diagnostics.iter().zip(expected) {
        assert_eq!(diagnostic.span.start, source.find(expected).unwrap());
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected
        );
    }
}

#[test]
fn generic_implicit_call_tearoffs_use_applicable_extension_methods() {
    assert_lints(
        "omit-local-variable-types",
        r#"
class C {}
extension on C { T call<T>(T x) => x; }
void use(C c) {
  var inferred = c<int>;
  int Function(int) typed = c<int>; /* expect: omit-local-variable-types */
  int Function(int) explicit = c.call<int>; /* expect: omit-local-variable-types */
  print([inferred, typed, explicit]);
}
"#,
        2,
    );
}

#[test]
fn numeric_await_static_tearoff_function_join_and_nullable_parameter_types() {
    assert_lints(
        "omit-local-variable-types",
        include_str!("support/residual_contexts/joins.dart"),
        16,
    );
}

#[test]
fn incomparable_parameter_types_join_through_never() {
    let source = include_str!("support/residual_contexts/meets.dart");
    assert_lints("omit-local-variable-types", source, 1);
    assert_lints("avoid-types-on-closure-parameters", source, 2);
}

#[test]
fn function_variance_and_direct_versus_nested_collection_callbacks() {
    assert_lints(
        "avoid-types-on-closure-parameters",
        include_str!("support/residual_contexts/closures.dart"),
        8,
    );
}

#[test]
fn lexical_generic_tearoffs_keep_expression_context_and_named_order() {
    assert_lints(
        "omit-local-variable-types",
        include_str!("support/residual_contexts/lexical.dart"),
        8,
    );
}

#[test]
fn compatible_optional_and_required_shapes_keep_variance() {
    assert_lints(
        "omit-local-variable-types",
        include_str!("support/residual_contexts/shapes.dart"),
        2,
    );
}

#[test]
fn optional_positional_shapes_require_concrete_downward_context() {
    assert_lints(
        "avoid-types-on-closure-parameters",
        include_str!("support/residual_contexts/handlers.dart"),
        4,
    );
}

#[test]
fn function_joins_keep_common_optional_prefix_and_constructor_shape() {
    let source = include_str!("support/residual_contexts/optional_prefix.dart");
    assert_lints("omit-local-variable-types", source, 8);
    assert_lints("avoid-types-on-closure-parameters", source, 1);
}

#[test]
fn joined_function_shapes_only_preserve_shared_typedef_provenance() {
    let source = include_str!("support/residual_contexts/joined_alias.dart");
    assert_lints("discarded-futures", source, 6);
    assert_lints("unawaited-futures", source, 6);
}

#[test]
fn prefixed_extensions_obey_show_and_hide_without_explicit_extension_access() {
    let inputs = [
        (
            "/project/lib/prefixed.dart",
            include_str!("support/residual_contexts/prefixed.dart"),
        ),
        (
            "/project/lib/extensions.dart",
            include_str!("support/residual_contexts/extensions.dart"),
        ),
        (
            "/project/lib/hidden.dart",
            include_str!("support/residual_contexts/hidden.dart"),
        ),
    ];
    let parsed: Vec<_> = inputs
        .iter()
        .map(|(path, source)| {
            let result = parse(source);
            assert!(result.errors.is_empty(), "{path}: {:?}", result.errors);
            std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: (*source).into(),
                tree: result.tree,
            })
        })
        .collect();
    let sources: Vec<_> = inputs
        .iter()
        .zip(&parsed)
        .map(|((path, _), parsed)| IdentitySource {
            path: Path::new(path),
            parsed,
            has_parse_errors: false,
        })
        .collect();
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files: Vec<_> = sources
        .iter()
        .zip(&parsed)
        .map(|(source, parsed)| (source.path.to_path_buf(), parsed.clone()))
        .collect();
    let signatures = SignatureIndex::build(&files, None, &identities, &types);
    let rules = all_rules();
    let rule_name = "omit-local-variable-types";
    let rule = &rules.iter().find(|(name, _)| *name == rule_name).unwrap().1;
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(Path::new(inputs[0].0), &config, &parsed[0])
        .with_types(&types)
        .with_identities(&identities)
        .with_signatures(&signatures);
    let diagnostics = rule.analyze(parsed[0].program(), &context);
    assert_diagnostics(inputs[0].1, rule_name, &diagnostics, 4);
}

#[test]
fn null_aware_calls_keep_reached_member_annotations() {
    assert_lints(
        "discarded-futures",
        r#"
import 'package:meta/meta.dart';
class Controller {
  @awaitNotRequired
  Future<void> forward() async {}
  Future<void> required() async {}
}
class Holder { Controller get controller => Controller(); }
void use(Controller? controller, Holder? holder) {
  controller?.forward();
  holder?.controller.forward();
  controller?.required(); /* expect: discarded-futures */
}
"#,
        1,
    );
}

#[test]
fn bounded_iterables_keep_for_in_annotations() {
    assert_lints(
        "omit-local-variable-types",
        r#"
void bounded<E, T extends Iterable<E>>(T? elements) {
  if (elements == null) return;
  for (E element in elements) { print(element); }
}
void direct<E>(Iterable<E> elements) {
  for (E element in elements) { print(element); } /* expect: omit-local-variable-types */
}
"#,
        1,
    );
}

#[test]
fn assignment_context_preserves_generic_receiver_promotions() {
    assert_lints(
        "avoid-types-on-closure-parameters",
        r#"
class Rows<E> {
  T fold<T>(T initial, T Function(T, E) combine) => initial;
}
class Table<K, V> {
  Table.copy(Table other);
  Rows<K> get keys => Rows<K>();
}
class Metadata {
  Metadata._();
  factory Metadata(Table<String, Metadata>? table) {
    if (table == null) return Metadata._();
    table = Table.copy(table);
    var empty = Metadata._();
    var merged = table.keys.fold(empty, (Metadata value, key) => value); /* expect: avoid-types-on-closure-parameters */
    return merged;
  }
}
"#,
        1,
    );
}

#[test]
fn future_diagnostic_spans_distinguish_methods_from_function_callees() {
    let source = r#"
Future<void> function() async {}
class Receiver {
  Future<void> method() async {}
  Future<void> Function() get callback => function;
}
Future<void> use(Receiver receiver, (Future<void> Function(),) record) async {
  function();
  receiver.method();
  (function)();
  (receiver.method)();
  receiver.callback();
  record.$1();
  (record.$1)();
}
"#;
    let rules = all_rules();
    let rule = &rules
        .iter()
        .find(|(name, _)| *name == "unawaited-futures")
        .unwrap()
        .1;
    let diagnostics = inference::analyze(
        rule.as_ref(),
        Path::new("future_spans.dart"),
        source,
        &FalconConfig::default(),
    );
    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(
        actual,
        [
            "function",
            "method",
            "(function)",
            "(receiver.method)",
            "receiver.callback",
            "record.$1",
            "(record.$1)",
        ]
    );
}

#[test]
fn prefixed_future_callees_distinguish_values_from_functions_and_methods() {
    let directory = tempfile::tempdir().unwrap();
    let declarations = r#"
Future<void> function() async {}
Future<void> Function() callback = function;
Future<void> Function() get topGetter => function;
class C {
  static Future<void> Function() get staticGetter => function;
  static Future<void> Function() staticField = function;
  static Future<void> method() async {}
}
"#;
    std::fs::write(directory.path().join("members.dart"), declarations).unwrap();
    let source = r#"
import 'members.dart' as prefix;
Future<void> use() async {
  prefix.callback();
  prefix.topGetter();
  prefix.C.staticGetter();
  prefix.C.staticField();
  prefix.C.method();
  prefix.function();
}
"#;
    let path = directory.path().join("prefixed_spans.dart");
    std::fs::write(&path, source).unwrap();
    let rules = all_rules();
    let rule = &rules
        .iter()
        .find(|(name, _)| *name == "unawaited-futures")
        .unwrap()
        .1;
    let diagnostics = inference::analyze(rule.as_ref(), &path, source, &FalconConfig::default());
    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(
        actual,
        [
            "prefix.callback",
            "prefix.topGetter",
            "prefix.C.staticGetter",
            "prefix.C.staticField",
            "method",
            "function",
        ]
    );
}

#[test]
fn cascade_future_invocations_preserve_callee_spans_and_alias_exemptions() {
    let source = r"
import 'dart:async';
import 'package:meta/meta.dart';
@awaitNotRequired
typedef Exempt = Future<void> Function();
class Callable {
  @awaitNotRequired
  Future<void> call() async {}
}
class Holder {
  Future<void> Function() factory() => () async {};
  Exempt exempt() => () async {};
  Callable callable() => Callable();
  Callable operator [](int index) => Callable();
}
void use(Holder holder) {
  holder..factory()();
  holder..exempt()();
  holder..callable()();
  holder..[0]();
}
";
    let rules = all_rules();
    let rule = &rules
        .iter()
        .find(|(name, _)| *name == "discarded-futures")
        .unwrap()
        .1;
    let diagnostics = inference::analyze(
        rule.as_ref(),
        Path::new("main.dart"),
        source,
        &FalconConfig::default(),
    );
    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(
        actual,
        ["..factory()", "..callable()", "..[0]"],
        "{diagnostics:?}"
    );
}

#[test]
fn cascade_implicit_call_does_not_reference_the_call_member() {
    assert_lints(
        "unreachable-from-main",
        r"
void reachedOnlyThroughCall() {}
class Callable { void call() { reachedOnlyThroughCall(); } } /* expect: unreachable-from-main */
class Holder { Callable callback() => Callable(); }
void main() { Holder()..callback()(); }
",
        1,
    );
}
