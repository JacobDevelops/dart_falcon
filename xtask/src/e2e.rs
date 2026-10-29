use crate::support::{DiagnosticJson, build_falcon, prepare_checkout};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;
type Counts = BTreeMap<String, u64>;
const PRESETS: [&str; 3] = [
    "package:lints/recommended.yaml",
    "package:flutter_lints/flutter.yaml",
    "package:very_good_analysis/",
];

#[derive(Args)]
pub struct Options {
    /// Run the smaller PR corpus only.
    #[arg(long)]
    fast: bool,
    /// Rewrite selected snapshots, without accepting crashes or internal errors.
    #[arg(long)]
    update: bool,
    /// Run one manifest entry, allowing long corpus passes to be queued separately.
    corpus: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    id: String,
    url: String,
    revision: String,
    #[serde(default)]
    subpath: String,
    #[serde(default)]
    sparse: bool,
    #[serde(default)]
    allow_syntax_errors: bool,
    #[serde(default)]
    excludes: Vec<String>,
    fast: bool,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
struct Snapshot {
    revision: String,
    rules: Counts,
}

fn diagnostics(output: &Output) -> Result<Vec<DiagnosticJson>> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !matches!(output.status.code(), Some(0 | 1))
        || stderr.contains("panicked at")
        || stderr.contains("stack overflow")
    {
        return Err(format!("falcon crashed ({}): {stderr}", output.status).into());
    }
    if [
        "failed to read file",
        "failed to access input",
        "failed to walk directory",
    ]
    .iter()
    .any(|message| stderr.contains(message))
    {
        return Err(format!("falcon skipped corpus inputs: {stderr}").into());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid diagnostic JSON: {error}; stderr: {stderr}").into())
}

fn counts(diagnostics: &[DiagnosticJson]) -> Counts {
    let mut counts = Counts::new();
    for diagnostic in diagnostics {
        if let Some(count) = counts.get_mut(diagnostic.rule.as_str()) {
            *count += 1;
        } else {
            counts.insert(diagnostic.rule.clone(), 1);
        }
    }
    counts
}

fn failures(diagnostics: &[DiagnosticJson], allow_syntax_errors: bool) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            (diagnostic.rule == "syntax-error" && !allow_syntax_errors)
                || matches!(
                    diagnostic.rule.as_str(),
                    "rules-thread-error" | "internal-error" | "config" | "unmatched-path"
                )
        })
        .map(|diagnostic| {
            format!(
                "{}:{}: {}: {}",
                diagnostic.file_path,
                diagnostic.line.unwrap_or_default(),
                diagnostic.rule,
                diagnostic.message
            )
        })
        .collect()
}

fn diff(expected: &Counts, actual: &Counts) -> Vec<String> {
    expected
        .keys()
        .chain(actual.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|rule| {
            let before = expected.get(rule).copied().unwrap_or_default();
            let after = actual.get(rule).copied().unwrap_or_default();
            (before != after).then(|| format!("  {rule}: {before} -> {after}"))
        })
        .collect()
}

fn check(binary: &Path, root: &Path, target: &Path, config: &Path) -> Result<Vec<DiagnosticJson>> {
    let capture = tempfile::tempdir()?;
    let stdout = capture.path().join("stdout");
    let stderr = capture.path().join("stderr");
    let mut child = Command::new(binary)
        .current_dir(root)
        .arg("check")
        .arg(target)
        .args(["--format", "json", "--config"])
        .arg(config)
        .env("RUST_LOG", "warn")
        .env("RAYON_NUM_THREADS", "4")
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env_remove("PATH")
        .stdout(Stdio::from(fs::File::create(&stdout)?))
        .stderr(Stdio::from(fs::File::create(&stderr)?))
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(300) {
            child.kill()?;
            child.wait()?;
            return Err(format!("falcon timed out after 300s on {}", target.display()).into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    diagnostics(&Output {
        status,
        stdout: fs::read(stdout)?,
        stderr: fs::read(stderr)?,
    })
}

fn migration_smoke(
    binary: &Path,
    checkout: &Path,
    scratch: &Path,
    tested: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in WalkDir::new(checkout)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || entry.file_name() != ".git")
    {
        let entry = entry?;
        if entry.file_name() != "analysis_options.yaml" || !entry.file_type().is_file() {
            continue;
        }
        let source = fs::read_to_string(entry.path())?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&source)?;
        let include = yaml.get("include").and_then(serde_yaml::Value::as_str);
        let Some(preset) = PRESETS
            .iter()
            .find(|preset| include.is_some_and(|include| include.starts_with(**preset)))
        else {
            continue;
        };
        if tested.contains(*preset) {
            continue;
        }
        let config = scratch.join("falcon.json");
        let output = Command::new(binary)
            .current_dir(checkout)
            .args(["migrate", "--input"])
            .arg(entry.path())
            .args(["--write", "--output"])
            .arg(&config)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "migration failed for {}: {}",
                entry.path().display(),
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let diagnostics = check(binary, scratch, Path::new("smoke.dart"), &config)?;
        let errors = failures(&diagnostics, false);
        if !errors.is_empty() {
            return Err(
                format!("migrated {}: {}", entry.path().display(), errors.join("\n")).into(),
            );
        }
        eprintln!("  migrated {} ({preset})", entry.path().display());
        tested.insert((*preset).to_owned());
    }
    Ok(())
}

fn prepare_corpus_checkout(checkout: &Path, corpus: &Corpus) -> Result<()> {
    if corpus.sparse {
        fs::create_dir_all(checkout)?;
        let status = Command::new("git").arg("init").arg(checkout).output()?;
        if !status.status.success() {
            return Err(String::from_utf8_lossy(&status.stderr).into_owned().into());
        }
        let status = Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["sparse-checkout", "set", &corpus.subpath])
            .output()?;
        if !status.status.success() {
            return Err(String::from_utf8_lossy(&status.stderr).into_owned().into());
        }
    }
    prepare_checkout(checkout, &corpus.url, &corpus.revision)
}

pub fn run(options: &Options) -> Result<()> {
    let root = crate::workspace_root();
    let corpora: Vec<Corpus> = serde_json::from_str(include_str!("../e2e-corpus.json"))?;
    if let Some(id) = &options.corpus
        && !corpora
            .iter()
            .any(|corpus| &corpus.id == id && (!options.fast || corpus.fast))
    {
        return Err(format!("no selected corpus named {id}").into());
    }
    let snapshot_path = root.join("xtask/e2e-snapshots.json");
    let mut snapshots: BTreeMap<String, Snapshot> = if snapshot_path.exists() {
        serde_json::from_slice(&fs::read(&snapshot_path)?)?
    } else {
        BTreeMap::new()
    };
    let binary = build_falcon(true);
    let cache = root.join("target/e2e");
    fs::create_dir_all(&cache)?;
    let scratch = tempfile::tempdir()?;
    fs::write(scratch.path().join("smoke.dart"), "void main() {}\n")?;
    let mut errors = Vec::new();
    let mut migrations = BTreeSet::new();
    for corpus in corpora.iter().filter(|corpus| {
        (!options.fast || corpus.fast) && options.corpus.as_ref().is_none_or(|id| &corpus.id == id)
    }) {
        eprintln!("e2e {} @ {}", corpus.id, corpus.revision);
        let result = (|| -> Result<()> {
            let checkout = cache.join(&corpus.id);
            prepare_corpus_checkout(&checkout, corpus)?;
            let config = checkout.join(".falcon-e2e.json");
            let config_json = serde_json::json!({
                "files": {"includes": std::iter::once("**".to_owned())
                    .chain(corpus.excludes.iter().map(|pattern| format!("!{pattern}")))
                    .collect::<Vec<_>>()}
            });
            fs::write(&config, serde_json::to_vec(&config_json)?)?;
            let diagnostics = check(&binary, &checkout, &checkout.join(&corpus.subpath), &config)?;
            let actual = Snapshot {
                revision: corpus.revision.clone(),
                rules: counts(&diagnostics),
            };
            let report = cache.join(format!("{}-errors.txt", corpus.id));
            let internal = failures(&diagnostics, corpus.allow_syntax_errors);
            fs::write(&report, internal.join("\n"))?;
            if !internal.is_empty() {
                errors.push(format!(
                    "{}: {} parser/internal errors; repro: {} (first: {})",
                    corpus.id,
                    internal.len(),
                    report.display(),
                    internal[0]
                ));
            }
            eprintln!("  {} diagnostics", diagnostics.len());
            if options.update {
                snapshots.insert(corpus.id.clone(), actual);
                fs::write(
                    &snapshot_path,
                    format!("{}\n", serde_json::to_string_pretty(&snapshots)?),
                )?;
            } else if let Some(expected) = snapshots.get(&corpus.id) {
                if expected.revision != actual.revision || expected.rules != actual.rules {
                    errors.push(format!(
                        "{} snapshot drift (revision {} -> {}):\n{}",
                        corpus.id,
                        expected.revision,
                        actual.revision,
                        diff(&expected.rules, &actual.rules).join("\n")
                    ));
                }
            } else {
                errors.push(format!(
                    "{} has no snapshot; run cargo xtask e2e --update",
                    corpus.id
                ));
            }
            migration_smoke(&binary, &checkout, scratch.path(), &mut migrations)?;
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{}: {error}", corpus.id));
        }
    }
    if !options.fast && options.corpus.is_none() {
        for preset in PRESETS {
            if !migrations.contains(preset) {
                errors.push(format!("no successful migration smoke test for {preset}"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_drift_include_added_removed_and_changed_rules() {
        let diagnostics: Vec<DiagnosticJson> = serde_json::from_value(serde_json::json!([
            {"rule":"one", "message":"first"},
            {"rule":"one", "message":"second"},
            {"rule":"new", "message":"third"}
        ]))
        .unwrap();
        let actual = counts(&diagnostics);
        let expected = Counts::from([("one".into(), 1), ("removed".into(), 3)]);
        assert_eq!(
            diff(&expected, &actual),
            ["  new: 0 -> 1", "  one: 1 -> 2", "  removed: 3 -> 0"]
        );
    }

    #[test]
    fn parser_and_worker_errors_remain_fatal_beside_lints() {
        let diagnostics: Vec<DiagnosticJson> = serde_json::from_value(serde_json::json!([
            {"rule":"syntax-error", "message":"expected expression", "file_path":"bad.dart", "line":2},
            {"rule":"rules-thread-error", "message":"worker failed"},
            {"rule":"avoid-print", "message":"lint"}
        ])).unwrap();
        assert_eq!(super::failures(&diagnostics, true).len(), 1);
        let failures = failures(&diagnostics, false);
        assert_eq!(failures.len(), 2);
        assert!(failures[0].starts_with("bad.dart:2: syntax-error:"));
    }

    #[cfg(unix)]
    #[test]
    fn crash_exit_and_panic_stderr_cannot_be_snapshotted() {
        use std::os::unix::process::ExitStatusExt;
        let mut output = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"[]".to_vec(),
            stderr: Vec::new(),
        };
        assert!(diagnostics(&output).unwrap().is_empty());
        output.status = std::process::ExitStatus::from_raw(1 << 8);
        assert!(diagnostics(&output).is_ok());
        output.status = std::process::ExitStatus::from_raw(101 << 8);
        assert!(diagnostics(&output).is_err());
        output.status = std::process::ExitStatus::from_raw(0);
        output.stderr = b"thread 'worker' panicked at source.rs:12".to_vec();
        assert!(diagnostics(&output).is_err());
        output.stderr = b"WARN failed to read file path=invalid.dart".to_vec();
        assert!(diagnostics(&output).is_err());
        output.stderr.clear();
        output.stdout = b"not JSON".to_vec();
        assert!(diagnostics(&output).is_err());
    }
}
