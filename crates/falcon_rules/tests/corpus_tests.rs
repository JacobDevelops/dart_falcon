//! In-process golden-corpus validation.
//!
//! Runs every rule against its `tests/corpus/<rule-name>/` fixtures and asserts the
//! emitted diagnostics line up with the `/* expect: <rule-name> */` annotations. This is
//! the `cargo test` counterpart of the out-of-process `cargo xtask validate-rules` harness:
//! it needs no pre-built binary, so `cargo test -p falcon_rules` exercises every rule.
//!
//! Invariants enforced:
//!   * every corpus subdirectory maps to a registered rule (no orphan fixtures);
//!   * every `/* expect: */` annotation is matched by a diagnostic on the same line;
//!   * no diagnostic fires on a line without a matching annotation (no false positives);
//!   * `good.dart` files (zero annotations) produce zero diagnostics.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use falcon_analyze::{AnalyzeContext, ProjectFile, Rule};
use falcon_config::FalconConfig;
use falcon_dart_parser::parser::parse;
use falcon_rules::dart_code_linter::use_design_system_item::UseDesignSystemItem;
use falcon_rules::{all_project_rules, all_rules};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// 1-indexed line number for a byte offset (mirrors the xtask harness).
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

#[derive(Debug)]
struct Expectation {
    rule: String,
    line: usize,
}

/// Parse `/* expect: rule-name */` (and `, msg: "..."`) annotations. Only the rule id and
/// line are used for matching here; messages are documented in fixtures but not asserted.
fn parse_expectations(source: &str) -> Vec<Expectation> {
    let mut exps = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx + 1;
        let mut search = line;
        while let Some(start) = search.find("/* expect:") {
            let after = &search[start + "/* expect:".len()..];
            let Some(end) = after.find("*/") else { break };
            let annotation = after[..end].trim();
            let rule = annotation
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !rule.is_empty() {
                exps.push(Expectation {
                    rule,
                    line: line_num,
                });
            }
            search = &after[end + 2..];
        }
    }
    exps
}

/// Run a single rule over `source`; return (rule-id, line) per diagnostic.
fn run_rule(
    rule: &dyn Rule,
    file: &Path,
    source: &str,
    config: &FalconConfig,
) -> Vec<(String, usize)> {
    let (program, _errors) = parse(source);
    let ctx = AnalyzeContext {
        file_path: file,
        source,
        config,
    };
    rule.analyze(&program, &ctx)
        .into_iter()
        .map(|d| {
            (
                d.rule.to_string(),
                byte_offset_to_line(source, d.span.start),
            )
        })
        .collect()
}

/// Load the per-rule corpus config (`corpus/<rule>/config.json`, full falcon.json
/// shape) if present; config-gated rules like use-design-system-item need one to
/// fire at all. Falls back to the default config.
fn corpus_config(rule_dir: &Path) -> FalconConfig {
    let path = rule_dir.join("config.json");
    if path.exists() {
        falcon_config::load_config(&path)
            .unwrap_or_else(|e| panic!("invalid corpus config {}: {e}", path.display()))
    } else {
        FalconConfig::default()
    }
}

fn dart_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("dart"))
        .collect();
    files.sort();
    files
}

#[test]
fn corpus_matches_expectations() {
    let rules = all_rules();
    let by_name: HashMap<&str, &dyn Rule> = rules.iter().map(|r| (r.name(), r.as_ref())).collect();

    let corpus = corpus_dir();
    let mut rule_dirs: Vec<PathBuf> = fs::read_dir(&corpus)
        .expect("corpus dir must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    rule_dirs.sort();

    let mut failures: Vec<String> = Vec::new();
    let mut total_expectations = 0usize;
    let mut total_files = 0usize;

    let project_rule_names: std::collections::HashSet<&str> =
        all_project_rules().iter().map(|r| r.name()).collect();

    for dir in &rule_dirs {
        let rule_name = dir.file_name().unwrap().to_string_lossy().to_string();

        // Project (cross-file) rules keep their multi-file fixtures in a
        // `project/` subdirectory and are validated by the dedicated test below.
        if project_rule_names.contains(rule_name.as_str()) {
            continue;
        }

        // Invariant: every corpus directory must correspond to a registered rule.
        let Some(rule) = by_name.get(rule_name.as_str()) else {
            failures.push(format!(
                "ORPHAN corpus dir `{rule_name}` maps to no registered rule"
            ));
            continue;
        };

        let config = corpus_config(dir);
        for file in dart_files(dir) {
            total_files += 1;
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));

            let expectations: Vec<Expectation> = parse_expectations(&source)
                .into_iter()
                .filter(|e| e.rule == rule_name)
                .collect();
            total_expectations += expectations.len();

            let mut diag_lines: Vec<usize> = run_rule(*rule, &file, &source, &config)
                .into_iter()
                .filter(|(r, _)| *r == rule_name)
                .map(|(_, line)| line)
                .collect();

            // Greedily match each expectation to an as-yet-unused diagnostic on the same line.
            for exp in &expectations {
                if let Some(pos) = diag_lines.iter().position(|&l| l == exp.line) {
                    diag_lines.remove(pos);
                } else {
                    failures.push(format!(
                        "MISS  {}:{} — `{}` expected but not emitted",
                        file.display(),
                        exp.line,
                        exp.rule
                    ));
                }
            }
            // Any diagnostics left over are unannotated false positives.
            for line in diag_lines {
                failures.push(format!(
                    "EXTRA {}:{} — `{}` fired without a matching annotation",
                    file.display(),
                    line,
                    rule_name
                ));
            }
        }
    }

    assert!(
        total_expectations > 0,
        "corpus produced no expectations — runner is not exercising fixtures"
    );
    assert!(
        failures.is_empty(),
        "golden corpus validation failed ({} file(s), {} expectation(s)):\n{}",
        total_files,
        total_expectations,
        failures.join("\n")
    );
}

#[test]
fn use_design_system_item_fires_only_when_configured() {
    let src = r#"
import 'package:flutter/material.dart';

Widget build(BuildContext context) {
  return Container(child: Text('hi'));
}
"#;
    let (program, _errors) = parse(src);
    let rule = UseDesignSystemItem;

    // Without configuration the rule is a no-op.
    let default_cfg = FalconConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("t.dart"),
        source: src,
        config: &default_cfg,
    };
    assert!(
        rule.analyze(&program, &ctx).is_empty(),
        "rule must be a no-op without configuration"
    );

    // With a configured item it flags the disallowed construction and names the replacement.
    let configured: FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": {
            "rules": {
                "style": {
                    "use-design-system-item": {
                        "level": "warn",
                        "options": {
                            "items": [
                                { "class_name": "Container", "use_instead": "AppContainer" }
                            ]
                        }
                    }
                }
            }
        }
    }))
    .expect("valid config");
    let ctx = AnalyzeContext {
        file_path: Path::new("t.dart"),
        source: src,
        config: &configured,
    };
    let diags = rule.analyze(&program, &ctx);
    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic, got {diags:?}"
    );
    assert_eq!(diags[0].rule, "use-design-system-item");
    assert!(
        diags[0].message.contains("AppContainer") && diags[0].message.contains("Container"),
        "message should name the replacement: {}",
        diags[0].message
    );

    // Named constructors / static access on the type are out of Phase-1 scope: `Container.of(x)`
    // must NOT be flagged (it is a static call, not a construction).
    let static_src = "Widget f(BuildContext c) => Container.of(c);";
    let (static_program, _e) = parse(static_src);
    let ctx = AnalyzeContext {
        file_path: Path::new("t.dart"),
        source: static_src,
        config: &configured,
    };
    assert!(
        rule.analyze(&static_program, &ctx).is_empty(),
        "static access `Container.of(...)` must not be flagged"
    );
}

fn collect_dart_files_recursive(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_dart_files_recursive_inner(root, &mut out, limit);
    out
}

fn collect_dart_files_recursive_inner(dir: &Path, out: &mut Vec<PathBuf>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        if out.len() >= limit {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !matches!(name, ".dart_tool" | "build" | ".pub-cache" | ".direnv") {
                collect_dart_files_recursive_inner(&path, out, limit);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("dart") {
            out.push(path);
        }
    }
}

/// In-process validation of project (cross-file) rules against their multi-file
/// fixtures under `corpus/<rule>/project/`. Each fixture directory is run as one
/// unit; `/* expect: <rule> */` annotations are matched by (file, line).
#[test]
fn project_corpus_matches_expectations() {
    let corpus = corpus_dir();
    let config = FalconConfig::default();
    let mut failures: Vec<String> = Vec::new();
    let mut total_expectations = 0usize;

    for rule in all_project_rules() {
        let dir = corpus.join(rule.name()).join("project");
        assert!(
            dir.is_dir(),
            "project rule `{}` is missing its corpus/<rule>/project/ fixture",
            rule.name()
        );

        let files: Vec<ProjectFile> = dart_files(&dir)
            .into_iter()
            .map(|path| {
                let source = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                let (program, errors) = parse(&source);
                ProjectFile {
                    path,
                    source,
                    program,
                    has_parse_errors: !errors.is_empty(),
                }
            })
            .collect();

        // (file_path, line) expectations for this rule across all fixture files.
        let mut expectations: Vec<(String, usize)> = Vec::new();
        for f in &files {
            for exp in parse_expectations(&f.source) {
                if exp.rule == rule.name() {
                    total_expectations += 1;
                    expectations.push((f.path.to_string_lossy().into_owned(), exp.line));
                }
            }
        }

        let mut diag_keys: Vec<(String, usize)> = rule
            .analyze_project(&files, &config)
            .into_iter()
            .filter(|d| d.rule == rule.name())
            .map(|d| {
                (
                    d.file_path.clone(),
                    byte_offset_to_line(
                        files
                            .iter()
                            .find(|f| f.path.to_string_lossy() == d.file_path)
                            .map(|f| f.source.as_str())
                            .unwrap_or(""),
                        d.span.start,
                    ),
                )
            })
            .collect();

        for exp in &expectations {
            if let Some(pos) = diag_keys.iter().position(|k| k == exp) {
                diag_keys.remove(pos);
            } else {
                failures.push(format!(
                    "MISS  {}:{} — `{}` expected but not emitted",
                    exp.0,
                    exp.1,
                    rule.name()
                ));
            }
        }
        for key in diag_keys {
            failures.push(format!(
                "EXTRA {}:{} — `{}` fired without a matching annotation",
                key.0,
                key.1,
                rule.name()
            ));
        }
    }

    assert!(
        total_expectations > 0,
        "project corpus produced no expectations — runner is not exercising fixtures"
    );
    assert!(
        failures.is_empty(),
        "project corpus validation failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn all_rules_no_name_collisions() {
    let rules = all_rules();
    let mut seen = std::collections::HashSet::new();
    for rule in &rules {
        let name = rule.name();
        assert!(!name.is_empty(), "rule has empty name");
        assert!(seen.insert(name), "duplicate rule name: {name}");
    }
    // Sanity: we have at least 60 rules
    assert!(rules.len() >= 60, "expected ≥60 rules, got {}", rules.len());
}

#[test]
fn all_rules_run_jfit_20_files_no_panic() {
    let jfit_lib = Path::new("/home/jacob/Documents/Developer/jfit/apps/mobile/lib");
    if !jfit_lib.exists() {
        eprintln!("jfit corpus not found at {}, skipping", jfit_lib.display());
        return;
    }

    let dart_files: Vec<PathBuf> = collect_dart_files_recursive(jfit_lib, 20);
    assert!(!dart_files.is_empty(), "no dart files found in jfit corpus");

    let rules = all_rules();
    let config = FalconConfig::default();
    let mut total_diags = 0usize;

    for path in &dart_files {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let (program, _errors) = parse(&source);
        let ctx = AnalyzeContext {
            file_path: path,
            source: &source,
            config: &config,
        };
        for rule in &rules {
            let diags = rule.analyze(&program, &ctx);
            total_diags += diags.len();
        }
    }

    eprintln!(
        "jfit integration: {} files, {} total diagnostics across {} rules",
        dart_files.len(),
        total_diags,
        rules.len()
    );
    // At least one rule must fire somewhere across 20 real-world files
    assert!(
        total_diags > 0,
        "no diagnostics emitted on jfit corpus — rules may not be scanning"
    );
}

#[test]
fn all_rules_order_independence() {
    let snippet = r#"
class Foo {
  dynamic x;
  void doThing(a, b, c, d, e, f) {
    var result = null;
    if (result == null) {
      print(result!);
    }
  }
}
"#;
    let (program, _errors) = parse(snippet);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source: snippet,
        config: &config,
    };

    let rules = all_rules();
    let mid = rules.len() / 2;

    // Collect all diagnostics running the full set
    let all_diags: Vec<String> = rules
        .iter()
        .flat_map(|r| r.analyze(&program, &ctx))
        .map(|d| format!("{}:{}", d.rule, d.span.start))
        .collect();

    // Collect first-half then second-half diagnostics
    let first_half: Vec<String> = rules[..mid]
        .iter()
        .flat_map(|r| r.analyze(&program, &ctx))
        .map(|d| format!("{}:{}", d.rule, d.span.start))
        .collect();
    let second_half: Vec<String> = rules[mid..]
        .iter()
        .flat_map(|r| r.analyze(&program, &ctx))
        .map(|d| format!("{}:{}", d.rule, d.span.start))
        .collect();

    let mut combined = first_half;
    combined.extend(second_half);

    // Both must produce identical diagnostics (same rules, same input, no shared mutable state)
    let mut all_sorted = all_diags.clone();
    let mut combined_sorted = combined.clone();
    all_sorted.sort();
    combined_sorted.sort();
    assert_eq!(
        all_sorted, combined_sorted,
        "rule execution is not order-independent — possible shared mutable state"
    );
}
