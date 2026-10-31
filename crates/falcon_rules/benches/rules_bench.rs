use criterion::{BenchmarkId, Criterion};
use falcon_analyze::{
    AnalyzeContext, IdentitySource, ParsedFile, RuleAnalysis, build_semantic_indexes, library_unit,
};
use falcon_config::{FalconConfig, RuleConfiguration, RulePlainConfiguration};
use falcon_rules::{
    build_registry,
    meta::{RULE_METADATA, RuleKind},
};
use std::{hint::black_box, path::PathBuf, sync::Arc};

// 10 representative Dart 3.x snippets covering the major grammar productions.
// Copied from crates/falcon_dart_parser/benches/parser_bench.rs for consistency.
static SNIPPETS: &[&str] = &[
    // 1. Class with fields, constructor, method
    r"
import 'dart:core';

class User {
  final String name;
  final int age;
  final String? email;

  const User({required this.name, required this.age, this.email});

  String greet() => 'Hello, $name!';

  @override
  String toString() => 'User(name: $name, age: $age)';
}
",
    // 2. Sealed class + pattern matching
    r"
sealed class Shape {}
class Circle extends Shape { final double radius; Circle(this.radius); }
class Rect extends Shape { final double w, h; Rect(this.w, this.h); }

double area(Shape s) => switch (s) {
  Circle(:final radius) => 3.14159 * radius * radius,
  Rect(:final w, :final h) => w * h,
};
",
    // 3. Async/await with generics
    r"
import 'dart:async';

Future<List<T>> fetchAll<T>(List<Future<T>> futures) async {
  final results = <T>[];
  for (final f in futures) {
    results.add(await f);
  }
  return results;
}

Stream<int> countDown(int from) async* {
  for (var i = from; i >= 0; i--) {
    await Future.delayed(const Duration(milliseconds: 100));
    yield i;
  }
}
",
    // 4. Mixin + interface + enum
    r"
mixin Logging {
  void log(String msg) => print('[${runtimeType}] $msg');
}

abstract interface class Repository<T> {
  Future<T?> findById(String id);
  Future<void> save(T entity);
}

enum Status { active, inactive, pending;
  bool get isActive => this == Status.active;
}
",
    // 5. Collection comprehensions + typed literals
    r"
List<int> range(int n) => [for (var i = 0; i < n; i++) i];

Map<String, int> wordCount(List<String> words) => {
  for (final w in words) w: (words.where((x) => x == w).length),
};

Set<String> unique(List<String> xs) => <String>{...xs};

const config = <String, dynamic>{
  'host': 'localhost',
  'port': 5432,
  'ssl': true,
};
",
    // 6. Extension methods + null safety
    r"
extension StringExt on String {
  String get capitalised => isEmpty ? this : '${this[0].toUpperCase()}${substring(1)}';
  String? get nullIfEmpty => isEmpty ? null : this;
  List<String> words() => split(RegExp(r'\s+'));
}

extension IntExt on int {
  bool get isEven => this % 2 == 0;
  List<int> to(int end) => [for (var i = this; i <= end; i++) i];
}
",
    // 7. Record types + destructuring
    r"
typedef Point = (double x, double y);

(double, double) translate((double, double) p, double dx, double dy) {
  final (x, y) = p;
  return (x + dx, y + dy);
}

({String name, int age}) makeRecord(String n, int a) => (name: n, age: a);
",
    // 8. Factory constructor + static methods
    r"
class Result<T, E> {
  final T? _value;
  final E? _error;
  const Result.ok(T value) : _value = value, _error = null;
  const Result.err(E error) : _value = null, _error = error;

  bool get isOk => _value != null;

  T unwrap() {
    if (_value == null) throw StateError('called unwrap on Err');
    return _value!;
  }

  static Result<T, E> tryRun<T, E>(T Function() fn) {
    try {
      return Result.ok(fn());
    } catch (e) {
      return Result.err(e as E);
    }
  }
}
",
    // 9. Abstract class hierarchy + annotations
    r"
@immutable
abstract class Widget {
  const Widget({this.key});
  final Key? key;
  Element createElement();
}

@immutable
class StatelessWidget extends Widget {
  const StatelessWidget({super.key});

  @override
  Element createElement() => StatelessElement(this);

  Widget build(BuildContext context);
}
",
    // 10. Top-level getters/setters + late fields + typedef
    r"
typedef Callback<T> = void Function(T value);
typedef AsyncCallback = Future<void> Function();

late final _instance = AppConfig._internal();

AppConfig get instance => _instance;

class AppConfig {
  AppConfig._internal();

  late String _baseUrl;
  String get baseUrl => _baseUrl;
  set baseUrl(String v) {
    if (v.isEmpty) throw ArgumentError('baseUrl cannot be empty');
    _baseUrl = v;
  }
}
",
];

fn benchmark_config() -> FalconConfig {
    let mut config = FalconConfig::default();
    config.linter.rules.recommended = Some(false);
    for meta in RULE_METADATA {
        if matches!(meta.kind, RuleKind::File { .. }) {
            config
                .linter
                .rules
                .groups
                .entry(meta.group.as_str().to_owned())
                .or_default()
                .insert(
                    meta.name.to_owned(),
                    RuleConfiguration::Plain(RulePlainConfiguration::Warn),
                );
        }
    }
    config
}

/// Parsing, project facts, context construction and lazy context cache warmup are untimed.
/// Timed calls include per-file resolution/inference, walks and registry postprocessing.
/// No SDK/private corpus is loaded; package-domain rules may take their bail-out path.
fn bench_project_facts_rules(c: &mut Criterion) {
    let config = benchmark_config();
    let registry = build_registry(&config);
    assert_eq!(
        registry.rules().len(),
        RULE_METADATA
            .iter()
            .filter(|meta| matches!(meta.kind, RuleKind::File { .. }))
            .count(),
        "rules benchmark setup: every file rule must be enabled"
    );
    let mut files: Vec<_> = SNIPPETS
        .iter()
        .enumerate()
        .map(|(i, source)| {
            (
                PathBuf::from(format!("synthetic_{i}.dart")),
                Arc::new(ParsedFile::parse(*source)),
            )
        })
        .collect();
    files.extend([
        (
            PathBuf::from("semantic.dart"),
            Arc::new(ParsedFile::parse(
                "import 'semantic_types.dart';\n\
                 void cast(Animal animal) { print(animal as NotAnimal); }\n\
                 int? nullable(int? value) => value!;\n",
            )),
        ),
        (
            PathBuf::from("semantic_types.dart"),
            Arc::new(ParsedFile::parse("class Animal {} class NotAnimal {}")),
        ),
    ]);
    for (path, parsed) in &files {
        assert!(
            !parsed.has_parse_errors(),
            "rules benchmark setup: {} must parse without errors",
            path.display()
        );
    }
    let sources: Vec<_> = files
        .iter()
        .map(|(path, parsed)| IdentitySource {
            path,
            parsed,
            has_parse_errors: parsed.has_parse_errors(),
        })
        .collect();
    // Reuse production fact builders without environment, external libraries or companions.
    let (indexes, grouping) = build_semantic_indexes(&sources, &[], None, None);
    let parsed_files: Vec<_> = files.iter().map(|(_, parsed)| parsed).collect();
    let libraries: Vec<_> = (0..files.len())
        .map(|i| library_unit(&grouping, &parsed_files, i))
        .collect();
    let contexts: Vec<_> = files
        .iter()
        .zip(&libraries)
        .map(|((path, parsed), library)| {
            AnalyzeContext::new(path, &config, parsed)
                .with_types(&indexes.types)
                .with_identities(&indexes.identities)
                .with_signatures(&indexes.signatures)
                .with_library(library)
        })
        .collect();

    assert_representative_work(&registry, &contexts);

    // Standalone cost includes each rule's own semantic or structural walk, not just its visitor.
    let mut isolated = c.benchmark_group("standalone_rule_project_facts_warm");
    for (name, rule) in registry.rules() {
        isolated.bench_function(
            BenchmarkId::new(format!("analyze_{}_files", contexts.len()), name),
            |b| {
                b.iter(|| {
                    for ctx in &contexts {
                        black_box(rule.analyze(name, ctx.parsed().program(), ctx));
                    }
                });
            },
        );
    }
    isolated.finish();

    let mut shared = c.benchmark_group("shared_registry_project_facts_warm");
    shared.bench_function(
        format!("run_all_enabled_file_rules_{}_files", contexts.len()),
        |b| {
            b.iter(|| {
                for ctx in &contexts {
                    black_box(registry.run_all(ctx.parsed().program(), ctx));
                }
            });
        },
    );
    shared.finish();
}

fn assert_representative_work<'a>(
    registry: &falcon_analyze::RuleRegistry,
    contexts: &'a [AnalyzeContext<'a>],
) {
    let expected = [
        "avoid-print",
        "avoid-unrelated-type-casts",
        "unnecessary-null-checks",
    ];
    for name in expected {
        assert!(
            registry.rules().iter().any(|(enabled, _)| *enabled == name),
            "rules benchmark setup: representative rule {name} must be enabled"
        );
    }
    for ctx in contexts {
        let shared = registry.run_all(ctx.parsed().program(), ctx);
        assert!(
            shared.iter().all(|diagnostic| {
                diagnostic.rule != falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
            }),
            "rules benchmark setup: shared rules worker failed on {}: {shared:?}",
            ctx.file_path().display()
        );
        for (name, rule) in registry.rules() {
            if ctx.file_path() == std::path::Path::new("semantic.dart") && expected.contains(name) {
                let isolated = rule.analyze(name, ctx.parsed().program(), ctx);
                for (lane, diagnostics) in [("isolated", &isolated), ("shared", &shared)] {
                    assert_eq!(
                        diagnostics
                            .iter()
                            .filter(|diagnostic| diagnostic.rule == *name)
                            .count(),
                        1,
                        "rules benchmark setup: {lane} lane must emit exactly one {name} on semantic.dart; \
                         check enabled rules and project semantic facts"
                    );
                }
            }
        }
    }
}

fn main() {
    // Enter once, like production, so run_all does not time per-file worker handoffs.
    falcon_dart_parser::with_stack(|| {
        let mut criterion = Criterion::default().configure_from_args();
        bench_project_facts_rules(&mut criterion);
        criterion.final_summary();
    })
    .expect("rules benchmark setup or execution failed on the rules stack");
}
