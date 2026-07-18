use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_config::FalconConfig;
use falcon_dart_parser::parser::parse;
use falcon_rules::all_rules;
use std::path::Path;

// 10 representative Dart 3.x snippets covering the major grammar productions.
// Copied from crates/falcon_dart_parser/benches/parser_bench.rs for consistency.
static SNIPPETS: &[&str] = &[
    // 1. Class with fields, constructor, method
    r#"
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
"#,
    // 2. Sealed class + pattern matching
    r#"
sealed class Shape {}
class Circle extends Shape { final double radius; Circle(this.radius); }
class Rect extends Shape { final double w, h; Rect(this.w, this.h); }

double area(Shape s) => switch (s) {
  Circle(:final radius) => 3.14159 * radius * radius,
  Rect(:final w, :final h) => w * h,
};
"#,
    // 3. Async/await with generics
    r#"
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
"#,
    // 4. Mixin + interface + enum
    r#"
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
"#,
    // 5. Collection comprehensions + typed literals
    r#"
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
"#,
    // 6. Extension methods + null safety
    r#"
extension StringExt on String {
  String get capitalised => isEmpty ? this : '${this[0].toUpperCase()}${substring(1)}';
  String? get nullIfEmpty => isEmpty ? null : this;
  List<String> words() => split(RegExp(r'\s+'));
}

extension IntExt on int {
  bool get isEven => this % 2 == 0;
  List<int> to(int end) => [for (var i = this; i <= end; i++) i];
}
"#,
    // 7. Record types + destructuring
    r#"
typedef Point = (double x, double y);

(double, double) translate((double, double) p, double dx, double dy) {
  final (x, y) = p;
  return (x + dx, y + dy);
}

({String name, int age}) makeRecord(String n, int a) => (name: n, age: a);
"#,
    // 8. Factory constructor + static methods
    r#"
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
"#,
    // 9. Abstract class hierarchy + annotations
    r#"
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
"#,
    // 10. Top-level getters/setters + late fields + typedef
    r#"
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
"#,
];

/// Helper to run all rules against a source file.
fn run_all_rules_on_source(rules: &[Box<dyn Rule>], path: &Path, source: &str) {
    let (program, _) = parse(source);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext {
        file_path: path,
        source,
        config: &config,
        project: None,
        types: None,
        library: None,
    };
    for rule in rules {
        let _ = rule.analyze(&program, &ctx);
    }
}

/// Benchmark all rules against 20 synthetic snippets (cycling the 10 snippets).
fn bench_all_rules_synthetic(c: &mut Criterion) {
    let rules = all_rules();
    let files: Vec<&str> = SNIPPETS.iter().cycle().take(20).copied().collect();

    c.bench_function("all_rules_synthetic_20", |b| {
        b.iter(|| {
            for (i, src) in files.iter().enumerate() {
                let path = std::path::PathBuf::from(format!("synthetic_{}.dart", i));
                run_all_rules_on_source(&rules, &path, src);
            }
        });
    });
}

/// Benchmark all rules against up to 20 .dart files from the jfit corpus.
/// If the path doesn't exist, returns early without error.
fn bench_all_rules_jfit(c: &mut Criterion) {
    let rules = all_rules();
    let corpus_root = std::path::PathBuf::from("/home/jacob/Documents/Developer/jfit");
    let mobile_lib = corpus_root.join("apps/mobile/lib");
    let search_root = if mobile_lib.exists() {
        mobile_lib
    } else {
        corpus_root.clone()
    };

    if !search_root.exists() {
        return;
    }

    let files: Vec<(std::path::PathBuf, String)> = collect_dart_files(&search_root)
        .into_iter()
        .take(20)
        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|content| (p, content)))
        .collect();

    if files.is_empty() {
        return;
    }

    c.bench_function("all_rules_jfit_corpus", |b| {
        b.iter(|| {
            for (path, src) in &files {
                run_all_rules_on_source(&rules, path, src);
            }
        });
    });
}

/// Benchmark each rule individually against a single representative snippet.
fn bench_per_rule(c: &mut Criterion) {
    let rules = all_rules();
    let snippet = SNIPPETS[0];
    let path = std::path::PathBuf::from("benchmark.dart");
    let (program, _) = parse(snippet);
    let config = FalconConfig::default();

    for rule in &rules {
        let rule_name = rule.name().to_string();
        c.bench_with_input(BenchmarkId::new("rule", &rule_name), &rule_name, |b, _| {
            b.iter(|| {
                let ctx = AnalyzeContext {
                    file_path: &path,
                    source: snippet,
                    config: &config,
                    project: None,
                    types: None,
                    library: None,
                };
                let _ = rule.analyze(&program, &ctx);
            });
        });
    }
}

/// Recursively collect .dart files from a directory.
fn collect_dart_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !matches!(name, ".direnv" | ".dart_tool" | "build" | ".pub-cache") {
                out.extend(collect_dart_files(&path));
            }
        } else if path.extension().is_some_and(|e| e == "dart") {
            out.push(path);
        }
    }
    out
}

criterion_group!(
    benches,
    bench_all_rules_synthetic,
    bench_all_rules_jfit,
    bench_per_rule
);
criterion_main!(benches);
