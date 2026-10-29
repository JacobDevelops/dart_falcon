use crate::{
    corpus::{Expectation, cross_file_project_roots, ordinary_fixture_files, parse_expectations},
    support::{DiagnosticJson, FalconRunner, build_falcon, docgen_config_for, recursive_files},
    workspace_root,
};
use clap::Args;
use rayon::prelude::*;
use std::{
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
    line: Option<usize>,
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
    let raw_diags = falcon.run(file, config)?;
    Ok(compare_file_diagnostics(
        source,
        &raw_diags,
        threshold,
        rule_filter,
    ))
}

fn compare_file_diagnostics(
    source: &str,
    raw_diags: &[DiagnosticJson],
    threshold: f64,
    rule_filter: Option<&str>,
) -> FileResult {
    let all_exps = parse_expectations(source);
    let exps: Vec<Expectation> = all_exps
        .into_iter()
        .filter(|e| rule_filter.map(|r| e.rule == r).unwrap_or(true))
        .collect();

    let diags: Vec<(usize, &DiagnosticJson)> = raw_diags
        .iter()
        .enumerate()
        .filter(|(_, d)| rule_filter.map(|r| d.rule == r).unwrap_or(true))
        .collect();

    let mut matched_diag_indices = vec![false; raw_diags.len()];
    let mut matched_expectations = vec![false; exps.len()];
    // Claim exact message matches first, then fuzzy matches, then annotations
    // without messages. A broad annotation cannot steal a constrained match.
    for phase in 0..3 {
        for (expectation_index, exp) in exps.iter().enumerate() {
            if matched_expectations[expectation_index] || (phase < 2) != exp.expected_msg.is_some()
            {
                continue;
            }
            if let Some(&(diagnostic_index, _)) = diags.iter().find(|(index, diagnostic)| {
                !matched_diag_indices[*index]
                    && diagnostic.rule == exp.rule
                    && diagnostic.line == Some(exp.line)
                    && exp.expected_msg.as_ref().is_none_or(|message| {
                        if phase == 0 {
                            message == &diagnostic.message
                        } else {
                            fuzzy_match(message, &diagnostic.message) >= threshold
                        }
                    })
            }) {
                matched_diag_indices[diagnostic_index] = true;
                matched_expectations[expectation_index] = true;
            }
        }
    }
    let matched = matched_expectations
        .iter()
        .filter(|&&matched| matched)
        .count();
    let missed = exps
        .iter()
        .zip(&matched_expectations)
        .filter(|(_, matched)| !**matched)
        .map(|(expectation, _)| MissedExpectation {
            rule: expectation.rule.clone(),
            line: expectation.line,
        })
        .collect();

    let false_positives: Vec<UnexpectedDiagnostic> = raw_diags
        .iter()
        .enumerate()
        .filter(|(i, d)| {
            !matched_diag_indices[*i] && rule_filter.map(|r| d.rule == r).unwrap_or(true)
        })
        .map(|(_, d)| UnexpectedDiagnostic {
            rule: d.rule.clone(),
            line: d.line,
            message: d.message.clone(),
        })
        .collect();

    FileResult {
        expected: exps.len(),
        matched,
        missed,
        false_positives,
    }
}

/// Validate project fixtures together. Cross-file fixtures may contain
/// independent bad/good package roots; resolver fixtures form one project.
/// Expectations use full paths because packages intentionally reuse basenames.
fn validate_project_dir(
    project_dir: &Path,
    rule_name: &str,
    falcon: &FalconRunner,
    config: Option<&Path>,
    threshold: f64,
    split_projects: bool,
) -> (usize, usize, Vec<String>) {
    let expectations = project_expectations(project_dir, rule_name);

    let mut failures = Vec::new();
    if expectations.is_empty() && split_projects {
        failures.push(format!(
            "ERROR {} — project fixture for `{rule_name}` has no expectations",
            project_dir.display()
        ));
        return (0, 0, failures);
    }
    let mut diags = Vec::new();
    let roots = if split_projects {
        cross_file_project_roots(project_dir)
    } else {
        vec![project_dir.to_path_buf()]
    };
    for root in roots {
        // A failed invocation must not read as "the rule fired nowhere".
        match falcon.run(&root, config) {
            Ok(found) => diags.extend(found),
            Err(e) => failures.push(format!(
                "ERROR {} — falcon invocation failed: {e}",
                root.display()
            )),
        }
    }
    let mut remaining: Vec<_> = diags
        .iter()
        .filter(|diagnostic| diagnostic.rule == rule_name)
        .collect();

    let expected = expectations.len();
    let mut matched = 0usize;
    let mut matched_expectations = vec![false; expectations.len()];
    for phase in 0..3 {
        for (expectation_index, (path, exp)) in expectations.iter().enumerate() {
            if matched_expectations[expectation_index] || (phase < 2) != exp.expected_msg.is_some()
            {
                continue;
            }
            if let Some(position) = remaining.iter().position(|diagnostic| {
                diagnostic.file_path == *path
                    && diagnostic.line == Some(exp.line)
                    && exp.expected_msg.as_ref().is_none_or(|message| {
                        if phase == 0 {
                            message == &diagnostic.message
                        } else {
                            fuzzy_match(message, &diagnostic.message) >= threshold
                        }
                    })
            }) {
                remaining.remove(position);
                matched_expectations[expectation_index] = true;
                matched += 1;
            }
        }
    }
    let unmatched = expectations
        .iter()
        .zip(&matched_expectations)
        .filter(|(_, matched)| !**matched)
        .map(|(expectation, _)| expectation);
    for (path, exp) in unmatched {
        if let Some(position) = remaining.iter().position(|diagnostic| {
            diagnostic.file_path == *path && diagnostic.line == Some(exp.line)
        }) {
            let diagnostic = remaining.remove(position);
            failures.push(format!(
                "MESSAGE {path}:{} — `{rule_name}`\n  expected: {:?}\n    actual: {:?}",
                exp.line,
                exp.expected_msg.as_deref().unwrap(),
                diagnostic.message
            ));
        } else {
            failures.push(format!(
                "MISS  {path}:{} — rule `{rule_name}` expected but not emitted{}",
                exp.line,
                exp.expected_msg
                    .as_ref()
                    .map_or_else(String::new, |message| format!(": {message:?}"))
            ));
        }
    }
    for diagnostic in remaining {
        failures.push(format!(
            "EXTRA {}:{} — rule `{rule_name}` fired unexpectedly: {:?}",
            diagnostic.file_path,
            diagnostic
                .line
                .map_or_else(|| "unresolved".to_string(), |line| line.to_string()),
            diagnostic.message
        ));
    }
    (expected, matched, failures)
}

fn project_expectations(project_dir: &Path, rule_name: &str) -> Vec<(String, Expectation)> {
    let fixture_files = recursive_files(project_dir);
    let mut expectations: Vec<(String, Expectation)> = Vec::new();
    for file in &fixture_files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let key = file.to_string_lossy().into_owned();
        for exp in parse_expectations(&source) {
            if exp.rule == rule_name {
                expectations.push((key.clone(), exp));
            }
        }
    }
    expectations
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

    let resolver_dependent = falcon_rules::rule_requires_resolution(&rule_name);
    let dart_files = ordinary_fixture_files(rule_dir, resolver_dependent);

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
    validate_ordinary_fixtures(
        &dart_files,
        &rule_name,
        falcon,
        threshold,
        rule_config.as_deref(),
        &mut outcome,
    );
    validate_project_fixtures(
        rule_dir,
        &rule_name,
        falcon,
        threshold,
        rule_config.as_deref(),
        resolver_dependent,
        &mut outcome,
    );
    outcome
}

fn validate_ordinary_fixtures(
    dart_files: &[PathBuf],
    rule_name: &str,
    falcon: &FalconRunner,
    threshold: f64,
    rule_config: Option<&Path>,
    outcome: &mut RuleOutcome,
) {
    for dart_file in dart_files {
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
            Some(rule_name),
            rule_config,
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
                    fp.line
                        .map_or_else(|| "unresolved".to_string(), |line| line.to_string()),
                    fp.rule,
                    fp.message
                )
            }));
    }
}

fn validate_project_fixtures(
    rule_dir: &Path,
    rule_name: &str,
    falcon: &FalconRunner,
    threshold: f64,
    rule_config: Option<&Path>,
    resolver_dependent: bool,
    outcome: &mut RuleOutcome,
) {
    // Cross-file rules keep their fixtures in a `cross-file/` subdirectory,
    // run as one invocation over the whole directory.
    let cross_file_dir = rule_dir.join("cross-file");
    if cross_file_dir.is_dir() {
        let (expected, matched, failures) = validate_project_dir(
            &cross_file_dir,
            rule_name,
            falcon,
            rule_config,
            threshold,
            true,
        );
        outcome.files += 1;
        outcome.expected += expected;
        outcome.matched += matched;
        outcome.failures.extend(failures);
    }
    let project_dir = rule_dir.join("project");
    if resolver_dependent && project_dir.is_dir() {
        let (expected, matched, failures) = validate_project_dir(
            &project_dir,
            rule_name,
            falcon,
            rule_config,
            threshold,
            false,
        );
        outcome.files += 1;
        outcome.expected += expected;
        outcome.matched += matched;
        outcome.failures.extend(failures);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_messages_claim_exact_matches_before_fuzzy_and_unconstrained_matches() {
        let diagnostics: Vec<_> = ["Use `Foo`.", "Use `Bar`."]
            .into_iter()
            .map(|message| DiagnosticJson {
                rule: "sample".to_owned(),
                message: message.to_owned(),
                line: Some(1),
                col: Some(1),
                file_path: "bad.dart".to_owned(),
            })
            .collect();
        let result = compare_file_diagnostics(
            "x /* expect: sample */ /* expect: sample, msg: \"Use `Foo`.\" */",
            &diagnostics,
            1.0,
            Some("sample"),
        );
        assert_eq!(result.matched, 2);
        assert!(result.missed.is_empty());
        assert!(result.false_positives.is_empty());
        let result = compare_file_diagnostics(
            "x /* expect: sample, msg: \"Use `Baz`.\" */ /* expect: sample, msg: \"Use `Foo`.\" */",
            &diagnostics[..1],
            0.85,
            Some("sample"),
        );
        assert_eq!(result.matched, 1);
        assert_eq!(result.missed.len(), 1);
        assert!(result.false_positives.is_empty());
    }

    #[test]
    fn cross_file_fixture_without_expectations_fails_before_running_falcon() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
        let runner = FalconRunner {
            binary: directory
                .path()
                .join("missing-falcon")
                .to_string_lossy()
                .into_owned(),
            fallback_sdk: None,
        };
        let (expected, matched, failures) =
            validate_project_dir(directory.path(), "avoid-foo", &runner, None, 0.85, true);
        assert_eq!((expected, matched), (0, 0));
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("has no expectations"), "{failures:?}");
    }

    #[test]
    fn clean_resolver_project_still_runs_falcon() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
        let runner = FalconRunner {
            binary: directory
                .path()
                .join("missing-falcon")
                .to_string_lossy()
                .into_owned(),
            fallback_sdk: None,
        };
        let (expected, matched, failures) =
            validate_project_dir(directory.path(), "avoid-foo", &runner, None, 1.0, false);
        assert_eq!((expected, matched), (0, 0));
        assert_eq!(failures.len(), 1);
        assert!(
            failures[0].contains("falcon invocation failed"),
            "{failures:?}"
        );
    }

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
