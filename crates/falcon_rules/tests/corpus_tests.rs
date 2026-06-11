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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_config::{FalconConfig, RuleConfig};
use falcon_dart_parser::parser::parse;
use falcon_rules::all_rules;
use falcon_rules::dart_code_linter::use_design_system_item::UseDesignSystemItem;

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

/// Run a single rule over `source` with the default config; return (rule-id, line) per diagnostic.
fn run_rule(rule: &dyn Rule, file: &Path, source: &str) -> Vec<(String, usize)> {
    let (program, _errors) = parse(source);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext {
        file_path: file,
        source,
        config: &config,
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

    for dir in &rule_dirs {
        let rule_name = dir.file_name().unwrap().to_string_lossy().to_string();

        // Invariant: every corpus directory must correspond to a registered rule.
        let Some(rule) = by_name.get(rule_name.as_str()) else {
            failures.push(format!(
                "ORPHAN corpus dir `{rule_name}` maps to no registered rule"
            ));
            continue;
        };

        for file in dart_files(dir) {
            total_files += 1;
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));

            let expectations: Vec<Expectation> = parse_expectations(&source)
                .into_iter()
                .filter(|e| e.rule == rule_name)
                .collect();
            total_expectations += expectations.len();

            let mut diag_lines: Vec<usize> = run_rule(*rule, &file, &source)
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
    let mut options = HashMap::new();
    options.insert(
        "items".to_string(),
        serde_json::json!([{ "class_name": "Container", "use_instead": "AppContainer" }]),
    );
    let mut rules_cfg = HashMap::new();
    rules_cfg.insert(
        "use-design-system-item".to_string(),
        RuleConfig {
            enabled: true,
            options,
        },
    );
    let configured = FalconConfig {
        rules: rules_cfg,
        ..Default::default()
    };
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
