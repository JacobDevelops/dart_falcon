//! Exact syntax anchors checked against Dart 3.13.5 machine diagnostics.

#[path = "support/inference.rs"]
mod inference;

use std::path::Path;

use falcon_config::FalconConfig;
use falcon_rules::all_rules;

fn assert_ranges(source: &str, expected: &[(&str, &str)]) {
    assert_rule_ranges("always-specify-types", source, expected);
}

fn assert_rule_ranges(rule_name: &str, source: &str, expected: &[(&str, &str)]) {
    let rules = all_rules();
    let rule = &rules.iter().find(|(name, _)| *name == rule_name).unwrap().1;
    let diagnostics = inference::analyze(
        rule.as_ref(),
        Path::new("always_specify_types_ranges.dart"),
        source,
        &FalconConfig::default(),
    );
    let mut actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.span.start,
                &source[diagnostic.span.start..diagnostic.span.end],
            )
        })
        .collect();
    let mut expected: Vec<_> = expected
        .iter()
        .map(|(anchor, text)| (source.find(anchor).unwrap(), *text))
        .collect();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{source}");
}

#[test]
fn collection_literals_highlight_only_the_begin_token() {
    assert_ranges(
        r#"void main() {
  List<int> list = [1, 2];
  Set<int> set = {1, 2};
  Map<String, int> map = {'x': 1};
  List<int> constantList = const /* gap */ [1, 2];
  Set<int> constantSet = const {3, 4};
  Map<String, int> constantMap = const {'y': 2};
  List<List<int>> nested = [[3], <int>[4]];
  List<int> conditional = [if (true) 1, ...<int>[2]];
  Map<int, int> loop = {for (final entry in <int>[]) entry: entry};
}
"#,
        &[
            ("[1, 2];", "["),
            ("{1, 2};", "{"),
            ("{'x': 1}", "{"),
            ("const /* gap */", "const"),
            ("const {3", "const"),
            ("const {'y'", "const"),
            ("[[3]", "["),
            ("[3]", "["),
            ("[if (true)", "["),
            ("{for (final", "{"),
            ("final entry", "final"),
        ],
    );
}

#[test]
fn unmodified_parameters_highlight_metadata_modifiers_defaults_and_function_suffixes() {
    assert_ranges(
        r#"const int marker = 0;
class C {
  void method(covariant input) {}
  set value(covariant assigned) {}
  set decorated(@marker covariant decoratedInput) {}
}
void parameters(callback(arg), generic<T>(T arg), @marker decorated,
    [optional = 1, nullableCallback(item)? = null]) {}
void named({required argument, @marker required annotated}) {}
"#,
        &[
            ("covariant input", "covariant input"),
            ("covariant assigned", "covariant assigned"),
            (
                "@marker covariant decoratedInput",
                "@marker covariant decoratedInput",
            ),
            ("callback(arg)", "callback(arg)"),
            ("arg), generic", "arg"),
            ("generic<T>(T arg)", "generic<T>(T arg)"),
            ("@marker decorated,", "@marker decorated"),
            ("optional = 1", "optional = 1"),
            (
                "nullableCallback(item)? = null",
                "nullableCallback(item)? = null",
            ),
            ("item)?", "item"),
            ("required argument", "required argument"),
            ("@marker required annotated", "@marker required annotated"),
        ],
    );
}

#[test]
fn modified_parameters_highlight_only_the_type_replacing_keyword() {
    // These modifiers are accepted by the parser even where the analyzer also
    // reports EXTRANEOUS_MODIFIER; its lint still highlights the keyword.
    assert_ranges(
        r#"const int marker = 0;
void parameters(final modified, var mutable, @marker final annotated,
    [final optional = 1]) {}
class C {
  void method(covariant final modifiedMember) {}
  set value(covariant final assigned) {}
}
"#,
        &[
            ("final modified,", "final"),
            ("var mutable", "var"),
            ("final annotated", "final"),
            ("final optional", "final"),
            ("final modifiedMember", "final"),
            ("final assigned", "final"),
        ],
    );
}

#[test]
fn patterns_highlight_the_binding_name_or_its_own_keyword() {
    assert_ranges(
        r#"class Box<T> { final T value; const Box(this.value); }
void patterns(Object object) {
  var (left, right) = (1, 2);
  final [head, ...tail] = <int>[1, 2];
  if (object case (var first, final second)) {}
  if (object case Box<int>(:var value)) {}
  if (object case Box<int>(:final value)) {}
  if (object case Box<int>(:final int value)) {}
  if (object case Box<int>(value: var item)) {}
}
"#,
        &[
            ("left, right", "left"),
            ("right)", "right"),
            ("head,", "head"),
            ("tail]", "tail"),
            ("var first", "var"),
            ("final second", "final"),
            ("var value", "var"),
            ("final value", "final"),
            ("var item", "var"),
        ],
    );
}

#[test]
fn declaration_keywords_and_raw_generic_type_names_keep_their_full_ranges() {
    assert_ranges(
        r#"class Box<T> { const Box(); const Box.named(); }
var top = 1;
class C { static late final field = 2; }
void main() {
  late var local = 3;
  for (var element in <int>[]) {}
  for (final item in <int>[]) {}
  Box? nullable;
  Box<int> implicit = Box();
  Box<int> named = Box.named();
  Box<int> constant = const Box();
  Box<int> constantNamed = const Box.named();
  print(Box);
  switch (Object()) { case Box(): break; }
}
"#,
        &[
            ("var top", "var"),
            ("final field", "final"),
            ("var local", "var"),
            ("var element", "var"),
            ("final item", "final"),
            ("Box? nullable", "Box?"),
            ("Box();\n  Box<int> named", "Box"),
            ("Box.named();\n  Box<int> constant", "Box"),
            ("Box();\n  Box<int> constantNamed", "Box"),
            ("Box.named();\n  print", "Box"),
            ("Box);", "Box"),
            ("Box():", "Box"),
        ],
    );
}

#[test]
fn classic_loop_declarations_retain_their_keyword_outside_the_declaration_range() {
    assert_ranges(
        r#"void main() {
  for (var index = 0; index < 2; index++) {}
  for (final item = 0; item < 2;) { break; }
  List<int> values = <int>[for (var entry = 0; entry < 2; entry++) entry];
}
"#,
        &[
            ("var index", "var"),
            ("final item", "final"),
            ("var entry", "var"),
        ],
    );
}

#[test]
fn return_type_rules_highlight_operator_names_instead_of_the_operator_keyword() {
    let source = r#"const int marker = 0;
class C {
  operator +(int other) => this;
  operator ==(Object other) => false;
  operator [](int index) => 0;
  operator []=(int index, int value) {}
  @marker
  operator /* gap */ -(int other) => this;
}
"#;
    for rule in ["always-declare-return-types", "type-annotate-public-apis"] {
        assert_rule_ranges(
            rule,
            source,
            &[
                ("+(int", "+"),
                ("==(Object", "=="),
                ("[](int", "[]"),
                ("-(int", "-"),
            ],
        );
    }
}

#[test]
fn closure_parameters_use_the_same_keyword_and_complete_parameter_ranges() {
    assert_ranges(
        r#"void main() {
  Object callback = (final value, {required argument, flag = false}) => value;
}
"#,
        &[
            ("final value", "final"),
            ("required argument", "required argument"),
            ("flag = false", "flag = false"),
        ],
    );
}

#[test]
fn documented_parameters_include_the_last_attached_documentation_group() {
    let source = r#"const int marker = 0;
void f(
 /// The callback.
 callback,
 /** The value. */
 @marker value,
 /// Discarded.
 /** Reset. */
 /// After block.
 mixed,
 /// Plain docs.
 // intervening
 plain,
 [
 /// Optional.
 optional = 1,
 ]) {}
class C {
 set value(
 /// Setter docs.
 @marker covariant input) {}
 set typed(
 /// Typed docs.
 int input) {}
}
"#;
    for rule in ["always-specify-types", "type-annotate-public-apis"] {
        assert_rule_ranges(
            rule,
            source,
            &[
                ("/// The callback.", "/// The callback.\n callback"),
                ("/** The value. */", "/** The value. */\n @marker value"),
                ("/// After block.", "/// After block.\n mixed"),
                (
                    "/// Plain docs.",
                    "/// Plain docs.\n // intervening\n plain",
                ),
                ("/// Optional.", "/// Optional.\n optional = 1"),
                (
                    "/// Setter docs.",
                    "/// Setter docs.\n @marker covariant input",
                ),
            ],
        );
    }
}

#[test]
fn modified_setter_parameters_only_include_documentation_for_the_public_api_rule() {
    let source = "class C {\n set value(\n /// Keyword docs.\n final input) {}\n}\n";
    assert_ranges(source, &[("final input", "final")]);
    assert_rule_ranges(
        "type-annotate-public-apis",
        source,
        &[("/// Keyword docs.", "/// Keyword docs.\n final input")],
    );
}

#[test]
fn parameter_documentation_uses_metadata_precedence_before_extending_the_range() {
    let source = r#"const int marker = 0;
void f(
 /// a
 @marker
 /// b
 x,
 /// c
 @marker
 /// d
 @marker
 y,
 /// e
 @marker
 @marker
 z,
) {}
class C {
 set value(
 /// setter a
 @marker
 /// setter b
 input) {}
}
"#;
    for rule in ["always-specify-types", "type-annotate-public-apis"] {
        assert_rule_ranges(
            rule,
            source,
            &[
                ("@marker\n /// b", "@marker\n /// b\n x"),
                ("@marker\n /// d", "@marker\n /// d\n @marker\n y"),
                ("/// e", "/// e\n @marker\n @marker\n z"),
                ("@marker\n /// setter b", "@marker\n /// setter b\n input"),
            ],
        );
    }
}

#[test]
fn documented_closure_parameters_inside_interpolations_use_their_local_token_store() {
    assert_ranges(
        r#"void main() {
  String text = '${(
 /// interpolation
 value,
 ) => value}';
}
"#,
        &[("/// interpolation", "/// interpolation\n value")],
    );
}
