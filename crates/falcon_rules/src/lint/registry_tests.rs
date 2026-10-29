use falcon_analyze::AnalyzeContext;
use falcon_analyze::RuleAnalysis;
use falcon_dart_parser::parse;

#[test]
fn real_semantic_rules_match_standalone_diagnostics_with_suppressions() {
    use std::path::Path;

    use falcon_analyze::RuleRegistry;
    use falcon_config::FalconConfig;

    use crate::lint::correctness::{
        avoid_dynamic_calls::AvoidDynamicCalls, unnecessary_statements::UnnecessaryStatements,
        void_checks::VoidChecks,
    };
    use crate::lint::style::prefer_is_empty::PreferIsEmpty;

    let source = r"
class Value {}
void accept<T>(T value) {}
void f(dynamic target, Value value) {
  target.missing();
  // falcon-ignore lint/correctness/avoid-dynamic-calls: intentional dynamic dispatch
  target.suppressed();
  1 + 2;
  accept<void>(value);
  if (items.length == 0) return;
}
";
    let path = Path::new("/project/lib/main.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
    let config = FalconConfig::default();
    let context = indexes.context(path, &config, &parsed);

    // As in production, only enabled rules are registered.
    for enable_style in [false, true] {
        let mut registry = RuleRegistry::with_lookup(crate::meta::suppression_lookup);
        registry.register("void-checks", Box::new(VoidChecks));
        registry.register("avoid-dynamic-calls", Box::new(AvoidDynamicCalls));
        registry.register("unnecessary-statements", Box::new(UnnecessaryStatements));
        if enable_style {
            registry.register("prefer-is-empty", Box::new(PreferIsEmpty));
        }
        let mut expected: Vec<_> = registry
            .rules()
            .iter()
            .flat_map(|(name, rule)| rule.analyze(name, program, &context))
            .collect();
        normalize_diagnostics(&mut expected, &context);

        let actual = registry.run_all(program, &context);
        for name in [
            "void-checks",
            "avoid-dynamic-calls",
            "unnecessary-statements",
        ] {
            assert!(
                actual.iter().any(|diagnostic| diagnostic.rule == name),
                "{name}: {actual:?}"
            );
        }
        assert_eq!(
            actual
                .iter()
                .any(|diagnostic| diagnostic.rule == "prefer-is-empty"),
            enable_style,
        );
        assert_eq!(
            actual
                .iter()
                .filter(|diagnostic| diagnostic.rule == "avoid-dynamic-calls")
                .count(),
            1,
            "the suppressed dynamic call must not survive",
        );
        assert_equal_diagnostics(&actual, &expected);
    }
}

#[test]
fn real_structural_rules_match_standalone_diagnostics_with_suppressions() {
    use std::path::Path;

    use falcon_analyze::RuleRegistry;
    use falcon_config::FalconConfig;

    use crate::lint::correctness::{
        proper_from_environment::ProperFromEnvironment, throw_in_finally::ThrowInFinally,
    };
    use crate::lint::style::unnecessary_new::UnnecessaryNew;
    use crate::lint::suspicious::avoid_dynamic::AvoidDynamic;

    let source = structural_rule_source();
    let path = Path::new("/project/lib/structural.dart");
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, &config, &parsed);

    // Disabled rules must not join the shared pass or produce diagnostics.
    for enable_style in [false, true] {
        let mut registry = RuleRegistry::with_lookup(crate::meta::suppression_lookup);
        registry.register("avoid-dynamic", Box::new(AvoidDynamic));
        registry.register("proper-from-environment", Box::new(ProperFromEnvironment));
        registry.register("throw-in-finally", Box::new(ThrowInFinally));
        if enable_style {
            registry.register("unnecessary-new", Box::new(UnnecessaryNew));
        }
        let mut expected: Vec<_> = registry
            .rules()
            .iter()
            .flat_map(|(name, rule)| rule.analyze(name, program, &context))
            .collect();
        for (name, count) in [
            ("avoid-dynamic", 3),
            ("proper-from-environment", 3),
            ("throw-in-finally", 2),
            ("unnecessary-new", if enable_style { 10 } else { 0 }),
        ] {
            assert_eq!(
                expected
                    .iter()
                    .filter(|diagnostic| diagnostic.rule == name)
                    .count(),
                count,
                "standalone {name}: {expected:?}",
            );
        }
        normalize_diagnostics(&mut expected, &context);

        let actual = registry.run_all(program, &context);
        for (name, count) in [
            ("avoid-dynamic", 2),
            ("proper-from-environment", 2),
            ("throw-in-finally", 1),
            ("unnecessary-new", if enable_style { 9 } else { 0 }),
        ] {
            assert_eq!(
                actual
                    .iter()
                    .filter(|diagnostic| diagnostic.rule == name)
                    .count(),
                count,
                "registry {name}: {actual:?}",
            );
        }
        assert_structural_spans(source, &actual, enable_style);
        assert_equal_diagnostics(&actual, &expected);
    }
}

fn normalize_diagnostics(
    expected: &mut Vec<falcon_diagnostics::Diagnostic>,
    context: &AnalyzeContext<'_>,
) {
    let suppressions = falcon_analyze::FileSuppressions::parse(
        context.source(),
        &context.file_path().to_string_lossy(),
        crate::meta::suppression_lookup,
        context.tree(),
    );
    assert!(suppressions.diagnostics().is_empty());
    expected.retain(|diagnostic| {
        !suppressions.is_suppressed(
            diagnostic.rule,
            suppressions.line_for_offset(diagnostic.span.start),
        )
    });
    expected.sort_by(|left, right| {
        left.rule
            .cmp(right.rule)
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
            .then(left.message.cmp(&right.message))
    });
    expected.dedup_by(|left, right| {
        left.rule == right.rule && left.span == right.span && left.message == right.message
    });
}

fn assert_equal_diagnostics(
    actual: &[falcon_diagnostics::Diagnostic],
    expected: &[falcon_diagnostics::Diagnostic],
) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.rule, expected.rule);
        assert_eq!(actual.severity, expected.severity);
        assert_eq!(actual.message, expected.message);
        assert_eq!(actual.file_path, expected.file_path);
        assert_eq!(actual.span, expected.span);
        assert_eq!(actual.line, expected.line);
        assert_eq!(actual.col, expected.col);
    }
}

fn assert_structural_spans(
    source: &str,
    actual: &[falcon_diagnostics::Diagnostic],
    enable_style: bool,
) {
    let dynamic_offsets: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.rule == "avoid-dynamic")
        .map(|diagnostic| diagnostic.span.start)
        .collect();
    assert_eq!(
        dynamic_offsets,
        [
            source.find("dynamic>>").unwrap(),
            source.find("dynamic visible").unwrap(),
        ],
        "direct Map arguments are exempt, but nested and local types are visited",
    );
    let throws: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.rule == "throw-in-finally")
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(throws, ["throw new Box(8)"]);
    let environment_calls: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.rule == "proper-from-environment")
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(
        environment_calls,
        [
            "String.fromEnvironment('NESTED')",
            "String.fromEnvironment('VISIBLE')",
        ],
    );
    if enable_style {
        // Map exemptions, const-context routing, and finally function/try
        // boundaries are collector-local: ordinary expression callbacks
        // must still see constructors inside all of those subtrees.
        let constructions: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.rule == "unnecessary-new")
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            constructions,
            [
                "new Box(1)",
                "new Box(2)",
                "new Box(3)",
                "new Box(4)",
                "new Box(5)",
                "new Box(6)",
                "new Box(7)",
                "new Box(8)",
                "new Box(9)",
            ],
        );
    }
}

fn structural_rule_source() -> &'static str {
    r"
class Box {
  Box(int value);
}
void consume(Object first, Object second) {}
void f() {
  Map<dynamic, dynamic> map = <dynamic, dynamic>{new Box(1): new Box(2)};
  Map<String, List<dynamic>> nested = {};
  dynamic visible = new Box(3);
  // falcon-ignore lint/suspicious/avoid-dynamic: intentional escape hatch
  dynamic hidden = new Box(4);
  consume(new Box(5), String.fromEnvironment('NESTED'));
  final environment = String.fromEnvironment('VISIBLE');
  const constant = String.fromEnvironment('CONST');
  // falcon-ignore lint/correctness/proper-from-environment: intentional runtime fallback
  final fallback = String.fromEnvironment('SUPPRESSED');
  try {
    consume(visible, map);
  } finally {
    final callback = () => throw new Box(6);
    try {
      throw new Box(7);
    } catch (_) {}
    throw new Box(8);
    // falcon-ignore lint/correctness/throw-in-finally: intentional cleanup failure
    throw new Box(9);
  }
  // falcon-ignore lint/style/unnecessary-new: intentional explicit construction
  new Box(10);
}
"
}
