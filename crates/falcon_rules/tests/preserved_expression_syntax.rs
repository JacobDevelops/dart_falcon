//! SDK-free regressions for grouping and cascade selectors retained in syntax.

use std::path::Path;

use falcon_analyze::{AnalyzeContext, IdentityIndex, IdentitySource, Rule, RuleAnalysis};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::lint::correctness::avoid_unused_parameters::AvoidUnusedParameters;
use falcon_rules::lint::correctness::no_adjacent_strings_in_list::NoAdjacentStringsInList;
use falcon_rules::lint::correctness::null_check_on_nullable_type_parameter::NullCheckOnNullableTypeParameter;
use falcon_rules::lint::correctness::unnecessary_statements::UnnecessaryStatements;
use falcon_rules::lint::style::always_put_required_named_parameters_first::AlwaysPutRequiredNamedParametersFirst;
use falcon_rules::lint::style::prefer_asserts_with_message::PreferAssertsWithMessage;
use falcon_rules::lint::style::prefer_final_parameters::PreferFinalParameters;
use falcon_rules::lint::style::prefer_trailing_comma::PreferTrailingComma;
use falcon_rules::lint::style::unnecessary_await_in_return::UnnecessaryAwaitInReturn;
use falcon_rules::lint::style::unnecessary_null_checks::UnnecessaryNullChecks;
use falcon_rules::lint::style::unnecessary_parenthesis::UnnecessaryParenthesis;
use falcon_rules::lint::suspicious::no_equal_arguments::NoEqualArguments;

fn reports(rule: &dyn Rule, source: &str) -> Vec<String> {
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let path = Path::new("syntax.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    rule.analyze(program, &context)
        .into_iter()
        .map(|diagnostic| source[diagnostic.span.start..diagnostic.span.end].to_owned())
        .collect()
}

#[test]
fn comments_do_not_hide_grouping_or_create_grouping() {
    assert!(
        reports(
            &NoAdjacentStringsInList,
            "var values = [( /* note */ 'a' 'b' /* note */ )];"
        )
        .is_empty()
    );
    assert_eq!(
        reports(
            &NoAdjacentStringsInList,
            "var values = [ /* ( */ 'a' 'b' /* ) */ ];"
        ),
        ["'a' 'b'"]
    );
    assert_eq!(
        reports(
            &UnnecessaryParenthesis,
            "void f(int value) { print(( /* note */ value)); }"
        ),
        ["( /* note */ value)"]
    );
    assert_eq!(
        reports(&UnnecessaryStatements, "void f() { ( /* note */ f()); }"),
        ["( /* note */ f())"]
    );
}

#[test]
fn cascade_null_assertion_keeps_following_call_and_standalone_tearoffs_distinct() {
    let source = "class C { C? parent; void clear() {} void go(C c) {} }\n\
                  void f(C c) { c..clear()..parent!.go(c); c.go; c..go; }";
    assert_eq!(reports(&UnnecessaryStatements, source), ["c.go", "..go"]);
}

#[test]
fn legacy_cases_and_wildcard_patterns_retain_parentheses() {
    assert_eq!(
        reports(
            &UnnecessaryParenthesis,
            "// @dart=2.19\nvoid f(int x) { switch (x) { case (1): break; } }"
        ),
        ["(1)"]
    );
    assert_eq!(
        reports(&UnnecessaryParenthesis, "void f() { var (_) = ('a'); }"),
        ["('a')"]
    );
}

#[test]
fn extension_type_representation_formals_are_implicitly_final() {
    assert_eq!(
        reports(
            &PreferFinalParameters,
            "// @dart=3.12\nextension type E(int value) {\n\
             E.from(int input) : this(input);\n\
             int plus(int amount) => value + amount;\n\
             }"
        ),
        ["int input", "int amount"]
    );
    assert!(
        reports(
            &PreferFinalParameters,
            "// @dart=3.12\nextension type const E._((int, int) value) {}"
        )
        .is_empty()
    );
    assert!(
        reports(
            &PreferFinalParameters,
            "// @dart=3.13\nclass C(int value) { int plus(int amount) => amount; }"
        )
        .is_empty()
    );
}

#[test]
fn wildcard_pattern_grouping_does_not_hide_expression_grouping() {
    assert_eq!(
        reports(
            &UnnecessaryParenthesis,
            "int f(Object? value) => switch ((value)) { ((/* grouping */ _)) => (1) };"
        ),
        ["(value)", "(1)"]
    );
    assert!(
        reports(
            &UnnecessaryParenthesis,
            "void f(Object? value) { if (value case (_)) {} switch (value) { case (_): break; } }"
        )
        .is_empty()
    );
}

#[test]
fn legacy_typedefs_and_primary_constructors_reach_parameter_checks() {
    for source in [
        "typedef void Legacy({int? a, required int b});",
        "typedef Modern = void Function({int? a, required int b});",
        "// @dart=3.13\nclass C(int a, {required int b, int c = 0, required int d});",
    ] {
        assert_eq!(
            reports(&AlwaysPutRequiredNamedParametersFirst, source).len(),
            1,
            "{source}"
        );
    }
}

#[test]
fn primary_constructor_assertions_reach_constructor_checks() {
    let source = "// @dart=3.13\nclass C(final int value) { this : assert(value > 0); }";
    assert_eq!(reports(&PreferAssertsWithMessage, source).len(), 1);
}

#[test]
fn grouped_null_assertions_keep_their_nullable_context() {
    assert_eq!(
        reports(
            &UnnecessaryNullChecks,
            "class C {} void take(C? value) {} void f(C? value) { take(((value!))); C? copy = ((value!)); }"
        ),
        ["!", "!"]
    );
}

#[test]
fn grouped_positional_null_assertions_keep_their_type_parameter_context() {
    assert_eq!(
        reports(
            &NullCheckOnNullableTypeParameter,
            "void g<T>(T value) {} void f<T>(T? value) { g<T>(((value!))); }"
        ),
        ["!"]
    );
}

#[test]
fn grouped_arrow_awaits_are_reported_once_at_the_await_keyword() {
    let source = r#"
Future<int> plain(Future<int> value) async => await value;
Future<int> grouped(Future<int> value) async => (await value);
Future<int> nested(Future<int> value) async => ((await value));
class C {
  Future<int> method(Future<int> value) async => ((await value));
}
void outer(Future<int> value) {
  final callback = () async => ((await value));
}
Future<int> notReturned(Future<int> value) async => 1 + (await value);
"#;
    assert_eq!(
        reports(&UnnecessaryAwaitInReturn, source),
        ["await", "await", "await", "await", "await"]
    );
}

#[test]
fn primary_constructor_fields_are_not_unused_ordinary_parameters() {
    let source = r#"
// @dart=3.13
class Declaring(final int value) {
  this {}
}
class FieldInitializer(int value) {
  final int stored = value;
  this {}
}
class Ordinary {
  Ordinary(int unused) {}
}
"#;
    assert_eq!(reports(&AvoidUnusedParameters, source), ["unused"]);
}

#[test]
fn grouped_arrow_assertions_keep_their_nullable_context() {
    assert_eq!(
        reports(
            &UnnecessaryNullChecks,
            "class C {} C? f(C? value) => ((value!));"
        ),
        ["!"]
    );
    assert_eq!(
        reports(
            &NullCheckOnNullableTypeParameter,
            "T? f<T>(T? value) => ((value!));"
        ),
        ["!"]
    );
}

#[test]
fn nullable_type_parameter_diagnostics_select_terminal_operators() {
    // Analyzer-probed expression and pattern spans: length 1, columns 46 and 41.
    let source = r"T expression<T>(T? value) => ((value /* ! */ !));
void pattern<T>(T? value) {
  if (value case final captured /* ! */ !) {}
}
";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let path = Path::new("syntax.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    let diagnostics = NullCheckOnNullableTypeParameter.analyze(program, &context);
    let positions: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            let prefix = &source[..diagnostic.span.start];
            let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
            let column = prefix.rsplit('\n').next().unwrap().len() + 1;
            (
                line,
                column,
                &source[diagnostic.span.start..diagnostic.span.end],
            )
        })
        .collect();
    assert_eq!(positions, [(1, 46, "!"), (3, 41, "!")]);
}

#[test]
fn pattern_initializers_keep_non_expression_parent_exemptions() {
    assert!(
        reports(
            &UnnecessaryParenthesis,
            "Future<void> f(int a, int b, Object value, Future<(int, int)> Function() fetch) async {\n\
             var (sum) = (a + b);\n\
             var (cast) = (value as int);\n\
             var (check) = (value is int);\n\
             final (left, right) = (await fetch());\n\
             print([sum, cast, check, left, right]);\n\
             }"
        )
        .is_empty()
    );
}

#[test]
fn cascade_result_invocations_reach_argument_checks() {
    for source in [
        "void f(dynamic target, int value) { target..factory()(value, value); }",
        "void f(dynamic target, int value) { target..callbacks[0]<int>(value, value); }",
        "void f(dynamic target, int value) { target..factory()(nested(value, value)); }",
    ] {
        assert_eq!(
            reports(&NoEqualArguments, source),
            vec!["value"],
            "{source}"
        );
    }
}

#[test]
fn cascade_result_invocations_reach_grouped_arguments() {
    assert_eq!(
        reports(
            &UnnecessaryParenthesis,
            "void f(dynamic target, int value) { target..factory()((value)); }",
        ),
        vec!["(value)"],
    );
}

#[test]
fn cascade_result_invocations_reach_trailing_comma_checks() {
    assert_eq!(
        reports(
            &PreferTrailingComma,
            "void f(dynamic target, int value) { target..factory()(\nvalue\n); }",
        ),
        vec![")"],
    );
    assert!(
        reports(
            &PreferTrailingComma,
            "void f(dynamic target, int value) { target..factory()(\nvalue,\n); }",
        )
        .is_empty(),
    );
}
