use crate::{
    corpus::{Expectation, cross_file_project_roots, parse_expectations},
    support::{DiagnosticJson, FalconRunner, build_falcon, docgen_config_for, recursive_files},
    workspace_root,
};
use clap::Args;
use rayon::prelude::*;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(crate) struct Options {
    #[arg(
        long,
        help = "Corpus directory (default: crates/falcon_rules/tests/corpus)"
    )]
    corpus: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 0.85,
        help = "Fuzzy message match threshold, 0.0-1.0"
    )]
    threshold: f64,
    #[arg(long, help = "Filter to a single rule")]
    rule: Option<String>,
    #[arg(
        long,
        help = "Falcon binary (default: build in Cargo's target directory)"
    )]
    falcon_bin: Option<PathBuf>,
    #[arg(long, help = "Output results as JSON")]
    json: bool,
}

#[derive(Debug)]
struct MissedExpectation {
    rule: String,
    line: usize,
}

#[derive(Debug)]
struct UnexpectedDiagnostic {
    rule: String,
    line: usize,
    message: String,
}

#[derive(Debug)]
struct FileResult {
    expected: usize,
    matched: usize,
    missed: Vec<MissedExpectation>,
    false_positives: Vec<UnexpectedDiagnostic>,
}

fn fuzzy_match(a: &str, b: &str) -> f64 {
    strsim::jaro_winkler(a, b)
}

fn validate_file(
    file: &Path,
    source: &str,
    falcon: &FalconRunner,
    threshold: f64,
    rule_filter: Option<&str>,
    config: Option<&Path>,
) -> Result<FileResult, String> {
    let all_exps = parse_expectations(source);
    let exps: Vec<Expectation> = all_exps
        .into_iter()
        .filter(|e| rule_filter.map(|r| e.rule == r).unwrap_or(true))
        .collect();

    let raw_diags = falcon.run(file, config)?;
    let diags: Vec<(usize, &DiagnosticJson)> = raw_diags
        .iter()
        .enumerate()
        .filter(|(_, d)| rule_filter.map(|r| d.rule == r).unwrap_or(true))
        .collect();

    let mut matched_diag_indices: HashSet<usize> = HashSet::new();
    let mut missed: Vec<MissedExpectation> = Vec::new();
    let mut matched = 0usize;

    'exp_loop: for exp in &exps {
        for &(idx, diag) in &diags {
            if matched_diag_indices.contains(&idx) {
                continue;
            }
            if diag.rule != exp.rule || diag.line != exp.line {
                continue;
            }
            // Rule + line match. If expected_msg set, do fuzzy check.
            let msg_ok = exp
                .expected_msg
                .as_ref()
                .map(|em| fuzzy_match(em, &diag.message) >= threshold)
                .unwrap_or(true);
            if msg_ok {
                matched += 1;
                matched_diag_indices.insert(idx);
                continue 'exp_loop;
            }
        }
        missed.push(MissedExpectation {
            rule: exp.rule.clone(),
            line: exp.line,
        });
    }

    let false_positives: Vec<UnexpectedDiagnostic> = raw_diags
        .iter()
        .enumerate()
        .filter(|(i, d)| {
            !matched_diag_indices.contains(i) && rule_filter.map(|r| d.rule == r).unwrap_or(true)
        })
        .map(|(_, d)| UnexpectedDiagnostic {
            rule: d.rule.clone(),
            line: d.line,
            message: d.message.clone(),
        })
        .collect();

    Ok(FileResult {
        expected: exps.len(),
        matched,
        missed,
        false_positives,
    })
}

/// Validate each independent package under `corpus/<rule>/cross-file/` in its
/// own falcon invocation. Expectations use full paths because package fixtures
/// intentionally reuse basenames.
fn validate_cross_file_dir(
    cross_file_dir: &Path,
    rule_name: &str,
    falcon: &FalconRunner,
    config: Option<&Path>,
) -> (usize, usize, Vec<String>) {
    let fixture_files = recursive_files(cross_file_dir);
    let mut expectations: Vec<(String, usize)> = Vec::new();
    for file in &fixture_files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let key = file.to_string_lossy().into_owned();
        for exp in parse_expectations(&source) {
            if exp.rule == rule_name {
                expectations.push((key.clone(), exp.line));
            }
        }
    }

    let mut failures = Vec::new();
    let mut diags = Vec::new();
    for root in cross_file_project_roots(cross_file_dir) {
        // A failed invocation must not read as "the rule fired nowhere".
        match falcon.run(&root, config) {
            Ok(found) => diags.extend(found),
            Err(e) => failures.push(format!(
                "ERROR {} — falcon invocation failed: {e}",
                root.display()
            )),
        }
    }
    let mut diag_keys: Vec<(String, usize)> = diags
        .iter()
        .filter(|d| d.rule == rule_name)
        .map(|d| (d.file_path.clone(), d.line))
        .collect();

    let expected = expectations.len();
    let mut matched = 0usize;
    for exp in &expectations {
        if let Some(pos) = diag_keys.iter().position(|k| k == exp) {
            diag_keys.remove(pos);
            matched += 1;
        } else {
            failures.push(format!(
                "MISS  {}:{} — rule `{}` expected but not emitted",
                exp.0, exp.1, rule_name
            ));
        }
    }
    for key in diag_keys {
        failures.push(format!(
            "EXTRA {}:{} — rule `{}` fired unexpectedly",
            key.0, key.1, rule_name
        ));
    }
    (expected, matched, failures)
}

pub(crate) fn run(options: Options) -> Result<(), String> {
    let workspace = workspace_root();
    let Options {
        corpus: corpus_path,
        threshold,
        rule: rule_filter,
        falcon_bin,
        json: json_output,
    } = options;
    let corpus_path =
        corpus_path.unwrap_or_else(|| workspace.join("crates/falcon_rules/tests/corpus"));

    if !corpus_path.exists() {
        eprintln!(
            "error: corpus path does not exist: {}",
            corpus_path.display()
        );
        std::process::exit(1);
    }

    // Walk corpus/{rule_name}/*.dart
    let mut rule_dirs: Vec<PathBuf> = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            rule_filter
                .as_ref()
                .map(|r| p.file_name().and_then(|n| n.to_str()) == Some(r.as_str()))
                .unwrap_or(true)
        })
        .collect();
    rule_dirs.sort();
    if rule_dirs.is_empty()
        && let Some(rule) = &rule_filter
    {
        return Err(format!("no corpus fixtures match rule `{rule}`"));
    }
    let falcon_bin = falcon_bin.unwrap_or_else(|| build_falcon(false));
    let falcon = FalconRunner::new(falcon_bin, &workspace, &corpus_path);

    // Each rule spawns one falcon process per fixture; rules are independent.
    let outcomes: Vec<RuleOutcome> = rule_dirs
        .par_iter()
        .map(|rule_dir| validate_rule_dir(rule_dir, &corpus_path, &falcon, threshold))
        .collect();

    let mut total_files: usize = 0;
    let mut total_expected: usize = 0;
    let mut total_matched: usize = 0;
    let mut all_failures: Vec<String> = Vec::new();
    for outcome in outcomes {
        total_files += outcome.files;
        total_expected += outcome.expected;
        total_matched += outcome.matched;
        if !json_output {
            for msg in &outcome.failures {
                eprintln!("{msg}");
            }
        }
        all_failures.extend(outcome.failures);
    }

    if json_output {
        let report = serde_json::json!({
            "rules_checked": rule_dirs.len(),
            "files_checked": total_files,
            "expectations": total_expected,
            "matched": total_matched,
            "failures": all_failures,
            "pass": all_failures.is_empty(),
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!();
        println!("── validate-rules ──────────────────────────────────────");
        println!("  Rules checked:  {}", rule_dirs.len());
        println!("  Files checked:  {total_files}");
        println!("  Expectations:   {total_expected}");
        println!("  Matched:        {total_matched}");
        println!("  Failures:       {}", all_failures.len());
        if all_failures.is_empty() {
            println!("  Status:         PASS");
        } else {
            println!("  Status:         FAIL");
        }
        println!("────────────────────────────────────────────────────────");
    }

    if !all_failures.is_empty() {
        return Err(format!(
            "{} corpus validation failure(s)",
            all_failures.len()
        ));
    }
    Ok(())
}

#[derive(Default)]
struct RuleOutcome {
    files: usize,
    expected: usize,
    matched: usize,
    failures: Vec<String>,
}

fn validate_rule_dir(
    rule_dir: &Path,
    corpus_path: &Path,
    falcon: &FalconRunner,
    threshold: f64,
) -> RuleOutcome {
    let rule_name = rule_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut dart_files: Vec<PathBuf> = fs::read_dir(rule_dir)
        .unwrap_or_else(|_| panic!("Cannot read {}", rule_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("dart"))
        .collect();
    dart_files.sort();

    // Config-gated rules reuse their corpus config. Every other rule gets a
    // generated config so validation also exercises non-recommended rules.
    let mut outcome = RuleOutcome::default();
    let temp_directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => {
            outcome
                .failures
                .push(format!("cannot create temporary config directory: {error}"));
            return outcome;
        }
    };
    let rule_config = match falcon_rules::meta::RULE_METADATA
        .iter()
        .find(|meta| meta.name == rule_name)
        .map(|meta| {
            docgen_config_for(
                corpus_path,
                meta,
                &temp_directory.path().join("falcon.json"),
            )
        })
        .transpose()
    {
        Ok(config) => config,
        Err(error) => {
            outcome.failures.push(error);
            return outcome;
        }
    };
    for dart_file in &dart_files {
        let source = match fs::read_to_string(dart_file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("warning: cannot read {}: {}", dart_file.display(), e);
                continue;
            }
        };

        outcome.files += 1;
        let result = match validate_file(
            dart_file,
            &source,
            falcon,
            threshold,
            Some(&rule_name),
            rule_config.as_deref(),
        ) {
            Ok(result) => result,
            Err(error) => {
                outcome.failures.push(format!(
                    "ERROR {} — falcon invocation failed: {error}",
                    dart_file.display()
                ));
                continue;
            }
        };
        outcome.expected += result.expected;
        outcome.matched += result.matched;
        outcome.failures.extend(result.missed.iter().map(|m| {
            format!(
                "MISS  {}:{} — rule `{}` expected but not emitted",
                dart_file.display(),
                m.line,
                m.rule
            )
        }));
        outcome
            .failures
            .extend(result.false_positives.iter().map(|fp| {
                format!(
                    "EXTRA {}:{} — rule `{}` fired unexpectedly: {}",
                    dart_file.display(),
                    fp.line,
                    fp.rule,
                    fp.message
                )
            }));
    }

    // Cross-file rules keep their fixtures in a `cross-file/` subdirectory,
    // run as one invocation over the whole directory.
    let cross_file_dir = rule_dir.join("cross-file");
    if cross_file_dir.is_dir() {
        let (expected, matched, failures) =
            validate_cross_file_dir(&cross_file_dir, &rule_name, falcon, rule_config.as_deref());
        outcome.files += 1;
        outcome.expected += expected;
        outcome.matched += matched;
        outcome.failures.extend(failures);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_rule_filter_fails_before_building_or_running_falcon() {
        let directory = tempfile::tempdir().unwrap();
        let error = run(Options {
            corpus: Some(directory.path().to_path_buf()),
            threshold: 0.85,
            rule: Some("misspelled-rule".to_string()),
            falcon_bin: None,
            json: false,
        })
        .unwrap_err();
        assert!(
            error.contains("no corpus fixtures match rule `misspelled-rule`"),
            "{error}"
        );
    }
    #[test]
    fn clean_fixture_does_not_hide_a_missing_binary() {
        let directory = tempfile::tempdir().unwrap();
        let runner = super::FalconRunner {
            binary: directory
                .path()
                .join("missing-falcon")
                .to_string_lossy()
                .into_owned(),
            fallback_sdk: None,
        };
        let error = super::validate_file(
            &directory.path().join("clean.dart"),
            "void main() {}",
            &runner,
            1.0,
            None,
            None,
        )
        .unwrap_err();
        assert!(error.contains("Failed to run falcon binary"), "{error}");
    }
}
