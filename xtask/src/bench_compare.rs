use crate::support::prepare_checkout;
use clap::Args;
use falcon_rules::meta::{RULE_METADATA, RuleKind, RuleSource};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const MAX_RSS_KB: u32 = 8 * 1024 * 1024;
const EXCLUDED: &[&str] = &["**/.dart_tool/**", "**/build/**", "**/.?*/**", "**/.?*"];
type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

#[derive(Args)]
pub struct Options {
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(2..))]
    runs: u32,
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(2..))]
    cold_runs: u32,
    #[arg(long, default_value_t = 1)]
    warmups: u32,
    /// Direct Dart SDK executable, not a mise shim.
    #[arg(long)]
    dart: Option<PathBuf>,
    #[arg(long)]
    hyperfine: Option<PathBuf>,
    /// Direct DCM executable (defaults to the activated local installation).
    #[arg(long)]
    dcm: Option<PathBuf>,
    /// Direct DCL executable or Dart snapshot.
    #[arg(long)]
    dcl: Option<PathBuf>,
    /// Benchmark only these corpus ids; requires an explicit output path.
    #[arg(long, value_delimiter = ',', requires = "output")]
    corpus: Vec<String>,
    /// Report destination (defaults to the published website data).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Reuse an already built target/release/falcon.
    #[arg(long)]
    skip_build: bool,
}

struct Corpus {
    id: &'static str,
    repo: &'static str,
    rev: &'static str,
    directory: &'static str,
    included_directories: &'static [&'static str],
}

const CORPORA: &[Corpus] = &[
    Corpus {
        id: "dio",
        repo: "cfug/dio",
        rev: "f1bef462289b40992d625d4eb2a2f42321217573",
        directory: "dio",
        included_directories: &[],
    },
    Corpus {
        id: "bloc",
        repo: "felangel/bloc",
        rev: "b9be1e252ee24ffbe9f2fb629edc7624b2cfbf05",
        directory: "packages/bloc",
        included_directories: &[],
    },
    Corpus {
        id: "flame",
        repo: "flame-engine/flame",
        rev: "141d574d8ee0fef196c981f5ed52ddd92ef72052",
        directory: ".",
        included_directories: &[],
    },
    Corpus {
        id: "flutter-plugins",
        repo: "flutter/packages",
        rev: "759473e08b8ba4766bf1d6356761cf9e2a900f88",
        directory: "packages",
        included_directories: &[
            "packages/camera",
            "packages/google_maps_flutter",
            "packages/webview_flutter",
        ],
    },
];

fn output(command: &mut Command) -> Result<Output> {
    let result = command.output()?;
    if !result.status.success() {
        return Err(format!("{command:?}: {}", String::from_utf8_lossy(&result.stderr)).into());
    }
    Ok(result)
}

fn text(command: &mut Command) -> Result<String> {
    Ok(String::from_utf8(output(command)?.stdout)?
        .trim()
        .to_owned())
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn recorded_command(executable: &Path, args: &[String], project: &Path) -> String {
    std::iter::once(quote(Path::new(executable.file_name().unwrap_or_default())))
        .chain(args.iter().map(|arg| {
            let path = Path::new(arg);
            quote(path.strip_prefix(project).unwrap_or(path))
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

fn entries(root: &Path) -> impl Iterator<Item = walkdir::Result<walkdir::DirEntry>> {
    WalkDir::new(root).into_iter().filter_entry(|entry| {
        entry.depth() == 0
            || entry
                .file_name()
                .to_str()
                .is_none_or(|name| !name.starts_with('.') && name != "build")
    })
}

fn process_gone(error: &std::io::Error) -> bool {
    // Linux can return ESRCH after a process exits with its /proc file still open.
    error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(3)
}

fn resident_kb(pid: u32) -> Result<u64> {
    let status = match fs::read_to_string(format!("/proc/{pid}/status")) {
        Ok(status) => status,
        Err(error) if process_gone(&error) => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let kilobytes = |key| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default()
    };
    let swapped = kilobytes("VmSwap:");
    if swapped > 0 {
        return Err(format!(
            "benchmark process {pid} has {swapped} KiB swapped; discard this pass"
        )
        .into());
    }
    let mut total = kilobytes("VmRSS:");
    visit_children(pid, |child| {
        total += resident_kb(child)?;
        Ok(())
    })?;
    Ok(total)
}

fn visit_children(pid: u32, mut visit: impl FnMut(u32) -> Result<()>) -> Result<()> {
    let tasks = match fs::read_dir(format!("/proc/{pid}/task")) {
        Ok(tasks) => tasks,
        Err(error) if process_gone(&error) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for task in tasks {
        let task = match task {
            Ok(task) => task,
            Err(error) if process_gone(&error) => continue,
            Err(error) => return Err(error.into()),
        };
        let children = match fs::read_to_string(task.path().join("children")) {
            Ok(children) => children,
            Err(error) if process_gone(&error) => continue,
            Err(error) => return Err(error.into()),
        };
        for child in children.split_whitespace() {
            visit(child.parse()?)?;
        }
    }
    Ok(())
}

fn kill_tree(pid: u32) -> Result<()> {
    let signal = |name| {
        Command::new("kill")
            .args([name, "--", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
    };
    signal("-STOP")?;
    let mut child_error = None;
    let children = visit_children(pid, |child| {
        if let Err(error) = kill_tree(child) {
            child_error.get_or_insert(error);
        }
        Ok(())
    });
    let killed = signal("-KILL");
    if !matches!(&killed, Ok(status) if status.success()) {
        let _ = signal("-CONT");
    }
    let killed = killed?;
    children?;
    if !killed.success() && Path::new(&format!("/proc/{pid}")).exists() {
        return Err(format!("could not terminate benchmark process {pid}").into());
    }
    if let Some(error) = child_error {
        return Err(error);
    }
    Ok(())
}

fn guarded_output(command: &mut Command, capture: &Path, limit_kb: u64) -> Result<Output> {
    let stdout = capture.with_extension("stdout");
    let stderr = capture.with_extension("stderr");
    let mut child = command
        .stdout(fs::File::create(&stdout)?)
        .stderr(fs::File::create(&stderr)?)
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(error) => break Err(error.to_string()),
        }
        match resident_kb(child.id()) {
            Ok(kb) if kb > limit_kb => {
                break Err(
                    "benchmark process tree exceeded its resident-memory safety limit".to_owned(),
                );
            }
            Ok(_) => {}
            Err(error) => break Err(error.to_string()),
        }
        if start.elapsed() > Duration::from_mins(15) {
            break Err("benchmark invocation exceeded the 15-minute safety limit".to_owned());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if status.is_err() {
        let killed = kill_tree(child.id());
        if killed.is_err() {
            let _ = child.kill();
        }
        child.wait()?;
        killed?;
    }
    match status {
        Ok(status) => Ok(Output {
            status,
            stdout: fs::read(stdout)?,
            stderr: fs::read(stderr)?,
        }),
        Err(error) => Err(format!("{command:?}: {error}").into()),
    }
}

fn swap_counts() -> Result<[u64; 2]> {
    let contents = fs::read_to_string("/proc/vmstat")?;
    let count = |key| -> Result<u64> {
        let value = contents
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .ok_or("missing Linux swap counter")?;
        Ok(value.trim().parse()?)
    };
    Ok([count("pswpin ")?, count("pswpout ")?])
}

fn falcon_includes(excluded: &[String]) -> Vec<String> {
    excluded.iter().map(|path| format!("!./{path}")).collect()
}

fn configuration_for(dcl: bool) -> (Value, Vec<&'static str>) {
    let mut config = json!({
        "linter": {"rules": {"recommended": false}},
        "cross-file": {"rules": {"recommended": false}}
    });
    let mut names = Vec::with_capacity(RULE_METADATA.len());
    for rule in RULE_METADATA {
        let name = match rule.source {
            RuleSource::Lints(name) if !dcl => name,
            RuleSource::DartCodeLinter(name)
                if dcl
                    && !DCL_COMMANDS.contains(&name)
                    && !DCM_EXCLUSIONS.iter().any(|(id, _)| *id == name) =>
            {
                name
            }
            _ => continue,
        };
        names.push(name);
        let section = if matches!(rule.kind, RuleKind::CrossFile { .. }) {
            "cross-file"
        } else {
            "linter"
        };
        let rules = config[section]["rules"]
            .as_object_mut()
            .expect("rules object");
        rules
            .entry(rule.group.as_str())
            .or_insert_with(|| json!({}))[rule.name] = json!("warn");
    }
    names.sort_unstable();
    names.dedup();
    (config, names)
}

const DCL_COMMANDS: &[&str] = &[
    "check-unused-code",
    "check-unused-files",
    "check-unnecessary-nullable",
];

// Fixed eligibility policy observed with DCM 1.40.0 Free against the pinned bloc corpus.
// Exclude these upstream ids from all three tools, never just from DCM.
const DCM_EXCLUSIONS: &[(&str, &str)] = &[
    (
        "avoid-ignoring-return-values",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    ("format-comment", "Unavailable with DCM 1.40.0 Free license"),
    (
        "no-equal-arguments",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    (
        "no-object-declaration",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    (
        "prefer-correct-identifier-length",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    (
        "prefer-correct-type-name",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    (
        "prefer-trailing-comma",
        "Unavailable with DCM 1.40.0 Free license",
    ),
    (
        "use-design-system-item",
        "Unknown rule and unavailable with DCM 1.40.0 Free license",
    ),
];

fn excluded_dcl_rules() -> Value {
    Value::Array(DCM_EXCLUSIONS.iter().map(|(id, reason)| {
        json!({"id": id, "reason": reason, "source": "dcm", "appliesTo": ["falcon", "dcl", "dcm"]})
    }).collect())
}

fn configuration() -> (Value, Vec<&'static str>) {
    configuration_for(false)
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn metrics_report(bytes: &[u8]) -> Result<Value> {
    let start = bytes
        .iter()
        .position(|byte| *byte == b'{')
        .ok_or("missing DCL JSON report")?;
    let report: Value = serde_json::from_slice(&bytes[start..])?;
    if !report["formatVersion"].is_number() || !report["records"].is_array() {
        return Err("expected versioned DCL JSON report with records array".into());
    }
    Ok(report)
}

fn linter_diagnostics(bytes: &[u8]) -> Result<(usize, BTreeMap<String, usize>)> {
    let mut counts = BTreeMap::new();
    let report = metrics_report(bytes)?;
    for record in report["records"].as_array().ok_or("missing records")? {
        for issue in record["issues"].as_array().ok_or("missing record issues")? {
            let code = issue["ruleId"].as_str().ok_or("missing issue ruleId")?;
            *counts.entry(code.to_lowercase()).or_default() += 1;
        }
    }
    Ok((counts.values().sum(), counts))
}

fn metrics_diagnostics(bytes: &[u8]) -> Result<(usize, BTreeMap<String, usize>)> {
    let content = std::str::from_utf8(bytes)?;
    if content.contains("Config errors:")
        || content.contains("unknown-rule :")
        || content.contains("not-available-rule :")
    {
        return Err("DCM reports unsupported rules or subscription restrictions; refusing to publish unequal rule sets".into());
    }
    let mut counts = BTreeMap::new();
    for line in content.lines() {
        if let Some((code, _)) = line.trim().split_once(" : https://dcm.dev/docs/rules/") {
            *counts.entry(code.to_owned()).or_default() += 1;
        }
    }
    let total: usize = counts.values().sum();
    let reported = if content.contains("no issues found!") {
        0
    } else {
        content
            .lines()
            .find_map(|line| {
                line.split_once("total lint issues - ")
                    .map(|(_, summary)| summary)
            })
            .and_then(|summary| summary.split_whitespace().next())
            .ok_or("missing DCM console issue summary")?
            .parse::<usize>()?
    };
    if total != reported {
        return Err(format!("DCM console parser counted {total} issues but summary reports {reported}; refusing incomplete diagnostics").into());
    }
    Ok((total, counts))
}

fn diagnostics(tool: &str, bytes: &[u8]) -> Result<(usize, BTreeMap<String, usize>)> {
    let mut counts = BTreeMap::new();
    if tool == "falcon" {
        let value: Value = serde_json::from_slice(bytes)?;
        let items = value
            .as_array()
            .ok_or("expected falcon diagnostics array")?;
        for item in items {
            let code = item["rule"]
                .as_str()
                .or_else(|| item["code"].as_str())
                .ok_or("missing diagnostic rule")?;
            *counts.entry(code.to_owned()).or_default() += 1;
        }
        Ok((items.len(), counts))
    } else if tool == "dcl" {
        linter_diagnostics(bytes)
    } else if tool == "dcm" {
        metrics_diagnostics(bytes)
    } else {
        let content = std::str::from_utf8(bytes)?;
        for line in content.lines() {
            let fields: Vec<_> = line.split('|').collect();
            if fields.len() >= 8 {
                *counts.entry(fields[2].to_lowercase()).or_default() += 1;
            }
        }
        Ok((counts.values().sum(), counts))
    }
}

fn comparable_counts(
    tool: &str,
    counts: &BTreeMap<String, usize>,
    names: &[&str],
) -> (BTreeMap<String, usize>, usize) {
    let mut comparable = BTreeMap::<String, usize>::new();
    for (code, total) in counts {
        let mapped = if tool == "falcon" {
            RULE_METADATA
                .iter()
                .find(|rule| rule.name == code)
                .and_then(|rule| match rule.source {
                    RuleSource::Lints(name) => Some(name.to_lowercase()),
                    RuleSource::DartCodeLinter(name) if names.contains(&name) => {
                        Some(name.to_lowercase())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| code.clone())
        } else {
            code.clone()
        };
        *comparable.entry(mapped).or_default() += total;
    }
    let lint_count = comparable
        .iter()
        .filter(|(code, _)| names.iter().any(|name| name.eq_ignore_ascii_case(code)))
        .map(|(_, count)| count)
        .sum();
    (comparable, lint_count)
}

struct BenchmarkContext<'a> {
    root: &'a Path,
    cache: &'a Path,
    dart: &'a PathBuf,
    flutter: &'a Path,
    hyperfine: &'a Path,
    falcon: &'a PathBuf,
    options: &'a Options,
    config: &'a Value,
    names: &'a [&'static str],
    dcl_config: &'a Value,
    dcl_names: &'a [&'static str],
    dcl: &'a Path,
    dcm: &'a Path,
}

struct CorpusInventory {
    files: usize,
    lines: usize,
    locks: BTreeMap<String, String>,
}

struct Preflight {
    peak_rss_mb: f64,
    count: usize,
    lint_count: usize,
    errors: usize,
    comparable: BTreeMap<String, usize>,
}

struct Timing {
    raw: Value,
    swap_before: [u64; 2],
    swap_after: [u64; 2],
}

struct Invocation<'a, 'b> {
    benchmark: &'a BenchmarkContext<'b>,
    corpus: &'a Corpus,
    project: &'a Path,
    state: &'a Path,
    scenario: &'a str,
    cold: bool,
    runs: u32,
    tool: &'a str,
    executable: &'a Path,
    args: &'a [String],
    names: &'a [&'static str],
}

fn restore_dependency_locks(
    project: &Path,
    lock_archive: &Path,
    lock_gzip: &Path,
    baseline: Option<&Value>,
) -> Result<()> {
    if baseline.is_some_and(|old| !old["dependencyLocksGzipBase64"].is_string()) {
        return Err(
            "previous report lacks dependency lock snapshots; cannot reproduce its resolution"
                .into(),
        );
    }
    if let Some(encoded) = baseline.and_then(|old| old["dependencyLocksGzipBase64"].as_str()) {
        let encoded_path = lock_archive.with_extension("b64");
        fs::write(&encoded_path, encoded)?;
        fs::write(
            lock_gzip,
            output(Command::new("base64").arg("--decode").arg(&encoded_path))?.stdout,
        )?;
        let locks: BTreeMap<String, String> = serde_json::from_slice(
            &output(Command::new("gzip").arg("-dc").arg(lock_gzip))?.stdout,
        )?;
        for entry in entries(project) {
            let entry = entry?;
            if entry.file_name() == "pubspec.lock"
                && !locks.contains_key(
                    entry
                        .path()
                        .strip_prefix(project)?
                        .to_string_lossy()
                        .as_ref(),
                )
            {
                fs::remove_file(entry.path())?;
            }
        }
        for (path, content) in locks {
            if !Path::new(&path)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                return Err("unsafe dependency lock path".into());
            }
            fs::write(project.join(path), content)?;
        }
    }
    Ok(())
}

fn resolve_dependencies(
    project: &Path,
    flutter: &Path,
    baseline: Option<&Value>,
) -> Result<(Vec<String>, Vec<Value>)> {
    let mut excluded: Vec<_> = EXCLUDED.iter().map(|path| (*path).to_owned()).collect();
    let mut setup_failures = baseline
        .and_then(|old| old["setupFailures"].as_array())
        .cloned()
        .unwrap_or_default();
    for failure in &setup_failures {
        let path = failure["directory"]
            .as_str()
            .ok_or("invalid setup failure path")?;
        if path.is_empty()
            || !Path::new(path)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err("unsafe setup failure path".into());
        }
        excluded.push(format!("{path}/**"));
    }
    let manifests: Vec<_> = entries(project)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_name() == "pubspec.yaml")
        .map(walkdir::DirEntry::into_path)
        .collect();
    for manifest in manifests {
        let directory = manifest.parent().ok_or("pubspec parent")?;
        if setup_failures.iter().any(|failure| {
            failure["directory"]
                .as_str()
                .is_some_and(|path| directory.starts_with(project.join(path)))
        }) {
            continue;
        }
        let source = fs::read_to_string(&manifest)?;
        if source.contains("resolution: workspace") {
            continue;
        }
        let mut get = Command::new(flutter);
        get.current_dir(directory).args(["pub", "get"]);
        if directory.join("pubspec.lock").exists() {
            get.arg("--enforce-lockfile");
        }
        let resolved = get.output()?;
        if !resolved.status.success() {
            let relative = directory
                .strip_prefix(project)?
                .to_string_lossy()
                .replace('\\', "/");
            if relative.is_empty() || baseline.is_some() {
                return Err(String::from_utf8_lossy(&resolved.stderr)
                    .into_owned()
                    .into());
            }
            excluded.push(format!("{relative}/**"));
            setup_failures.push(
                json!({"directory": relative, "error": String::from_utf8_lossy(&resolved.stderr)}),
            );
        }
    }
    Ok((excluded, setup_failures))
}

fn collect_corpus_inventory(project: &Path, setup_failures: &[Value]) -> Result<CorpusInventory> {
    let mut files = 0usize;
    let mut locks = BTreeMap::new();
    let mut lines = 0usize;
    for entry in entries(project) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if entry.file_name() == "pubspec.lock" {
                locks.insert(
                    entry
                        .path()
                        .strip_prefix(project)?
                        .to_string_lossy()
                        .into_owned(),
                    fs::read_to_string(entry.path())?,
                );
            }
            if entry.file_name() == "analysis_options.yaml" {
                fs::remove_file(entry.path())?;
            }
            if entry.path().extension().is_some_and(|ext| ext == "dart")
                && !setup_failures.iter().any(|failure| {
                    entry.path().starts_with(
                        project.join(failure["directory"].as_str().unwrap_or_default()),
                    )
                })
            {
                files += 1;
                lines += fs::read_to_string(entry.path())?.lines().count();
            }
        }
    }
    Ok(CorpusInventory {
        files,
        lines,
        locks,
    })
}

fn benchmark_corpus(
    benchmark: &BenchmarkContext<'_>,
    corpus: &Corpus,
    previous: Option<&Value>,
    results: &mut Vec<Value>,
) -> Result<Value> {
    let BenchmarkContext {
        root,
        cache,
        flutter,
        config,
        ..
    } = *benchmark;
    let checkout = cache.join(corpus.id);
    prepare_checkout(
        &checkout,
        &format!("https://github.com/{}.git", corpus.repo),
        corpus.rev,
    )?;
    if !corpus.included_directories.is_empty() {
        output(
            Command::new("git")
                .arg("-C")
                .arg(&checkout)
                .args(["sparse-checkout", "set", "--cone"])
                .args(corpus.included_directories),
        )?;
    }
    let project = checkout.join(corpus.directory);
    let lock_archive = cache.join(format!("{}-locks.json", corpus.id));
    let lock_gzip = lock_archive.with_extension("json.gz");
    let baseline = previous
        .and_then(|report| report["corpora"].as_array())
        .and_then(|corpora| {
            corpora
                .iter()
                .find(|old| old["id"] == corpus.id && old["rev"] == corpus.rev)
        });
    restore_dependency_locks(&project, &lock_archive, &lock_gzip, baseline)?;
    let (excluded, setup_failures) = resolve_dependencies(&project, flutter, baseline)?;
    let mut project_config = config.clone();
    project_config["files"] = json!({"includes": falcon_includes(&excluded)});
    let CorpusInventory {
        files,
        lines,
        locks,
    } = collect_corpus_inventory(&project, &setup_failures)?;
    if baseline.is_some_and(|old| {
        old["files"].as_u64() != Some(files as u64) || old["lines"].as_u64() != Some(lines as u64)
    }) {
        return Err(format!(
            "{}: source counts differ from the recorded corpus",
            corpus.id
        )
        .into());
    }
    let config_path = project.join("falcon.benchmark.json");
    write_json(&config_path, &project_config)?;
    let filter = serde_json::from_value(project_config["files"]["includes"].clone())?;
    std::env::set_current_dir(&project)?;
    let discovered = falcon_project::walk_project(&[PathBuf::from(".")], &filter, false)
        .map(|output| output.files.len());
    std::env::set_current_dir(root)?;
    let discovered = discovered?;
    if discovered != files {
        return Err(format!(
            "{}: falcon discovers {discovered} Dart files, corpus count is {files}",
            corpus.id
        )
        .into());
    }
    fs::write(&lock_archive, serde_json::to_vec(&locks)?)?;
    fs::write(
        &lock_gzip,
        output(Command::new("gzip").arg("-nc").arg(&lock_archive))?.stdout,
    )?;
    let encoded_locks = text(Command::new("base64").arg("-w0").arg(&lock_gzip))?;
    let metadata = json!({"id": corpus.id, "name": corpus.repo, "url": format!("https://github.com/{}", corpus.repo), "rev": corpus.rev, "directory": corpus.directory, "includedDirectories": corpus.included_directories, "files": files, "lines": lines, "setupFailures": setup_failures, "dependencyLocksGzipBase64": encoded_locks});
    measure_corpus(
        benchmark,
        corpus,
        &project,
        &config_path,
        &excluded,
        results,
    )?;
    Ok(metadata)
}

const DCL_FILE_EXCLUSIONS: &[&str] = &["**/*.g.dart", "**/*.freezed.dart", "**/*.mapper.dart"];
const DCL_EXCLUDE_GLOB: &str = "{**/*.g.dart,**/*.freezed.dart,**/*.mapper.dart}";

fn write_configurations(
    family: &str,
    names: &[&str],
    config: &Value,
    project: &Path,
    config_path: &Path,
    excluded: &[String],
) -> Result<()> {
    let generated = if family == "dcl-rules" {
        DCL_FILE_EXCLUSIONS
    } else {
        &[]
    };
    let mut yaml = String::from("analyzer:\n  exclude:\n");
    for path in excluded
        .iter()
        .map(String::as_str)
        .chain(generated.iter().copied())
    {
        writeln!(yaml, "    - '{path}'")?;
    }
    if family == "same-rules" {
        yaml.push_str("linter:\n  rules:\n");
        for name in names {
            writeln!(yaml, "    {name}: true")?;
        }
    } else {
        for section in ["dart_code_linter", "dcm"] {
            writeln!(yaml, "{section}:\n  rules:")?;
            for name in names {
                writeln!(yaml, "    - {name}")?;
            }
        }
    }
    fs::write(project.join("analysis_options.yaml"), yaml)?;
    let mut project_config = config.clone();
    let mut includes = falcon_includes(excluded);
    includes.extend(generated.iter().map(|path| format!("!./{path}")));
    project_config["files"] = json!({"includes": includes});
    write_json(config_path, &project_config)
}

fn error_count(tool: &str, bytes: &[u8], by_rule: &BTreeMap<String, usize>) -> Result<usize> {
    Ok(match tool {
        "dart-analyze" => std::str::from_utf8(bytes)?
            .lines()
            .filter(|line| line.starts_with("ERROR|"))
            .count(),
        "dcl" => metrics_report(bytes)?["records"]
            .as_array()
            .ok_or("missing records")?
            .iter()
            .filter_map(|record| record["issues"].as_array())
            .flatten()
            .filter(|issue| {
                issue["severity"]
                    .as_str()
                    .is_some_and(|severity| severity.eq_ignore_ascii_case("error"))
            })
            .count(),
        "dcm" => std::str::from_utf8(bytes)?
            .lines()
            .filter(|line| line.trim_start().starts_with("ERROR "))
            .count(),
        _ => *by_rule.get("syntax-error").unwrap_or(&0),
    })
}

impl BenchmarkContext<'_> {
    fn tool_command(&self, tool: &str, config_path: &Path) -> Result<(&Path, Vec<String>)> {
        Ok(match tool {
            "falcon" => (
                self.falcon.as_path(),
                vec![
                    "check".to_owned(),
                    ".".to_owned(),
                    "--config".to_owned(),
                    config_path.to_string_lossy().into_owned(),
                    "--format".to_owned(),
                    "json".to_owned(),
                    "--exit-code".to_owned(),
                    "0".to_owned(),
                ],
            ),
            "dart-analyze" => (
                self.dart.as_path(),
                vec![
                    "--suppress-analytics".to_owned(),
                    "analyze".to_owned(),
                    "--format=machine".to_owned(),
                    ".".to_owned(),
                ],
            ),
            "dcl" => {
                let mut args = Vec::new();
                let executable = if self.dcl.extension().is_some_and(|ext| ext == "snapshot") {
                    args.push(self.dcl.to_string_lossy().into_owned());
                    self.dart.as_path()
                } else {
                    self.dcl
                };
                args.extend([
                    "analyze".to_owned(),
                    "--reporter=json".to_owned(),
                    "--no-verbose".to_owned(),
                    "--exclude".to_owned(),
                    DCL_EXCLUDE_GLOB.to_owned(),
                    ".".to_owned(),
                ]);
                (executable, args)
            }
            "dcm" => (
                self.dcm,
                vec![
                    "analyze".to_owned(),
                    "--sdk-path".to_owned(),
                    self.dart
                        .parent()
                        .and_then(Path::parent)
                        .ok_or("SDK path")?
                        .to_string_lossy()
                        .into_owned(),
                    "--no-analytics".to_owned(),
                    "--no-global-version".to_owned(),
                    "--exclude".to_owned(),
                    DCL_EXCLUDE_GLOB.to_owned(),
                    ".".to_owned(),
                ],
            ),
            _ => unreachable!("fixed benchmark tool"),
        })
    }
}

fn measure_corpus(
    benchmark: &BenchmarkContext<'_>,
    corpus: &Corpus,
    project: &Path,
    config_path: &Path,
    excluded: &[String],
    results: &mut Vec<Value>,
) -> Result<()> {
    let BenchmarkContext { cache, options, .. } = *benchmark;
    for (family, names, config, tools) in [
        (
            "same-rules",
            benchmark.names,
            benchmark.config,
            &["falcon", "dart-analyze"][..],
        ),
        (
            "dcl-rules",
            benchmark.dcl_names,
            benchmark.dcl_config,
            &["falcon", "dcl", "dcm"][..],
        ),
    ] {
        write_configurations(family, names, config, project, config_path, excluded)?;
        for cold in [true, false] {
            let scenario = format!("{family}-{}", if cold { "cold" } else { "warm" });
            let runs = if cold {
                options.cold_runs
            } else {
                options.runs
            };
            for &tool in tools {
                let state = cache.join(format!("{}-{family}-{tool}-analyzer-state", corpus.id));
                let (executable, args) = benchmark.tool_command(tool, config_path)?;
                results.push(
                    Invocation {
                        benchmark,
                        corpus,
                        project,
                        state: &state,
                        scenario: &scenario,
                        cold,
                        runs,
                        tool,
                        executable,
                        args: &args,
                        names,
                    }
                    .measure()?,
                );
            }
        }
    }
    Ok(())
}

impl Invocation<'_, '_> {
    fn preflight(&self) -> Result<Preflight> {
        let Self {
            benchmark,
            corpus,
            project,
            state,
            scenario,
            cold,
            tool,
            executable,
            args,
            names,
            ..
        } = *self;
        let BenchmarkContext { cache, dart, .. } = *benchmark;
        if cold && state.exists() {
            fs::remove_dir_all(state)?;
        }
        let capture_path = cache.join(format!("{}-{scenario}-{tool}-preflight", corpus.id));
        let rss_path = capture_path.with_extension("rss");
        let mut capture = Command::new("/usr/bin/time");
        capture
            .args(["-q", "-f", "%M", "-o"])
            .arg(&rss_path)
            .arg(executable)
            .args(args)
            .current_dir(project)
            .env("ANALYZER_STATE_LOCATION_OVERRIDE", state)
            .env(
                "DART_SDK",
                dart.parent().and_then(Path::parent).ok_or("SDK path")?,
            );
        if tool == "dcm" {
            capture.env_remove("CI");
        }
        let captured = guarded_output(&mut capture, &capture_path, u64::from(MAX_RSS_KB))?;
        let peak_rss_mb =
            fs::read_to_string(&rss_path)?.trim().parse::<f64>()? * 1024.0 / 1_000_000.0;
        if !matches!(captured.status.code(), Some(code) if if tool == "falcon" { code == 0 } else { (0..=3).contains(&code) })
        {
            return Err(format!(
                "{tool} failed ({}): {}\n{}",
                captured.status,
                String::from_utf8_lossy(&captured.stdout),
                String::from_utf8_lossy(&captured.stderr)
            )
            .into());
        }
        fs::write(
            cache.join(format!("{}-{scenario}-{tool}-stderr.txt", corpus.id)),
            &captured.stderr,
        )?;
        let mut bytes = captured.stdout;
        if tool == "dart-analyze" {
            bytes.extend_from_slice(&captured.stderr);
        }
        fs::write(
            cache.join(format!("{}-{scenario}-{tool}-diagnostics.txt", corpus.id)),
            &bytes,
        )?;
        let (count, by_rule) = diagnostics(tool, &bytes).map_err(|error| {
            format!(
                "{tool} preflight: {error}; {}",
                String::from_utf8_lossy(&captured.stderr)
            )
        })?;
        if [
            "undefined_lint",
            "removed_lint",
            "replaced_lint",
            "unknown-config",
        ]
        .iter()
        .any(|code| by_rule.contains_key(*code))
        {
            return Err(format!("{tool} reports an unsupported rule or invalid configuration; inspect saved diagnostics").into());
        }
        let (comparable, lint_count) = comparable_counts(tool, &by_rule, names);
        let errors = error_count(tool, &bytes, &by_rule)?;
        Ok(Preflight {
            peak_rss_mb,
            count,
            lint_count,
            errors,
            comparable,
        })
    }

    fn time(&self) -> Result<Timing> {
        let Self {
            benchmark,
            corpus,
            project,
            state,
            scenario,
            cold,
            runs,
            tool,
            executable,
            args,
            ..
        } = *self;
        let BenchmarkContext {
            cache,
            dart,
            hyperfine,
            options,
            ..
        } = *benchmark;
        let command = format!(
            "{} {}",
            quote(executable),
            args.iter()
                .map(|arg| quote(Path::new(arg)))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let export = cache.join(format!("{}-{scenario}-{tool}.json", corpus.id));
        let mut timing = Command::new(hyperfine);
        timing
            .current_dir(project)
            .env("ANALYZER_STATE_LOCATION_OVERRIDE", state)
            .env(
                "DART_SDK",
                dart.parent().and_then(Path::parent).ok_or("SDK path")?,
            )
            .args([
                "--warmup",
                &options.warmups.to_string(),
                "--runs",
                &runs.to_string(),
                "--ignore-failure",
                "--export-json",
            ])
            .arg(&export);
        if tool == "dcm" {
            timing.env_remove("CI");
        }
        if cold {
            timing.args(["--prepare", &format!("rm -rf -- {}", quote(state))]);
        }
        let swap_before = swap_counts()?;
        let timed = guarded_output(timing.arg(&command), &export, u64::from(MAX_RSS_KB))?;
        let swap_after = swap_counts()?;
        if !timed.status.success() {
            return Err(format!(
                "hyperfine failed: {}",
                String::from_utf8_lossy(&timed.stderr)
            )
            .into());
        }
        let raw: Value = serde_json::from_slice(&fs::read(&export)?)?;
        Ok(Timing {
            raw,
            swap_before,
            swap_after,
        })
    }

    fn measure(&self) -> Result<Value> {
        let Self {
            corpus,
            project,
            scenario,
            runs,
            tool,
            executable,
            args,
            ..
        } = *self;
        let Preflight {
            peak_rss_mb,
            count,
            lint_count,
            errors,
            comparable,
        } = self.preflight()?;
        let Timing {
            raw,
            swap_before,
            swap_after,
        } = self.time()?;
        let measured = &raw["results"][0];
        let measurements = measured["measurements"]
            .as_array()
            .ok_or("missing measurements (hyperfine 2 required)")?;
        if measurements.iter().any(|sample| {
            sample["exit_code"].as_i64().is_none_or(|code| {
                if tool == "falcon" {
                    code != 0
                } else {
                    !(0..=3).contains(&code)
                }
            })
        }) {
            return Err("timed command failed unexpectedly".into());
        }
        let samples = measurements
            .iter()
            .map(|sample| {
                sample["time_wall_clock"]["value"]
                    .as_f64()
                    .map(|seconds| seconds * 1000.0)
                    .ok_or("invalid sample")
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let summary = &measured["summary"]["time_wall_clock"];
        Ok(
            json!({"corpus": corpus.id, "scenario": scenario, "tool": tool,
            "meanMs": summary["mean"].as_f64().ok_or("missing mean")? * 1000.0,
            "stddevMs": summary["stddev"].as_f64().ok_or("missing stddev")? * 1000.0,
            "minMs": summary["min"].as_f64().ok_or("missing min")? * 1000.0,
            "maxMs": summary["max"].as_f64().ok_or("missing max")? * 1000.0,
            "runs": runs, "peakRssMb": peak_rss_mb, "diagnostics": count, "lintDiagnostics": lint_count, "nonLintDiagnostics": count - lint_count, "errorDiagnostics": errors, "diagnosticsByRule": comparable,
            "swapPagesRead": swap_after[0] - swap_before[0], "swapPagesWritten": swap_after[1] - swap_before[1],
            "command": recorded_command(executable, args, project), "samplesMs": samples }),
        )
    }
}

fn dart_executable(options: &Options) -> Result<PathBuf> {
    if let Some(path) = &options.dart {
        return Ok(fs::canonicalize(path)?);
    }
    let wrapper = PathBuf::from(text(Command::new("mise").args([
        "x",
        "flutter@3.47.7-stable",
        "--",
        "which",
        "dart",
    ]))?);
    Ok(fs::canonicalize(
        wrapper
            .parent()
            .ok_or("Dart wrapper parent")?
            .join("cache/dart-sdk/bin/dart"),
    )?)
}

fn require_tool_version(tool: &str, actual: &str, expected: &str) -> Result<()> {
    if !actual
        .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .any(|part| part == expected)
    {
        return Err(format!(
            "expected {tool} {expected} for the pinned shared-rule eligibility policy, got {actual}",
        ).into());
    }
    Ok(())
}

fn competitor_tools(options: &Options, dart: &Path) -> Result<(PathBuf, PathBuf, String, String)> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?);
    let metrics_path = options
        .dcm
        .clone()
        .unwrap_or_else(|| home.join(".local/share/dcm/1.40.0/dcm"));
    let dcm = fs::canonicalize(&metrics_path).map_err(|error| {
        format!(
            "DCM executable not available at {}: {error}; install DCM or pass --dcm PATH",
            metrics_path.display(),
        )
    })?;
    let pub_cache = std::env::var_os("PUB_CACHE").map_or_else(
        || {
            let isolated = home.join(".local/share/falcon-bench-pub");
            if isolated
                .join("global_packages/dart_code_linter/bin/metrics.dart-3.13.5.snapshot")
                .is_file()
            {
                isolated
            } else {
                home.join(".pub-cache")
            }
        },
        PathBuf::from,
    );
    let linter_path = options.dcl.clone().unwrap_or_else(|| {
        pub_cache.join("global_packages/dart_code_linter/bin/metrics.dart-3.13.5.snapshot")
    });
    let dcl = fs::canonicalize(&linter_path).map_err(|error| format!(
        "DCL executable or snapshot not available at {}: {error}; activate dart_code_linter with Dart 3.13.5 or pass --dcl PATH",
        linter_path.display(),
    ))?;
    let mut command = if dcl.extension().is_some_and(|ext| ext == "snapshot") {
        let mut command = Command::new(dart);
        command.arg(&dcl);
        command
    } else {
        Command::new(&dcl)
    };
    let linter_release = text(command.arg("--version"))?;
    let metrics_release = text(Command::new(&dcm).env_remove("CI").arg("--version"))?;
    require_tool_version("DCL", &linter_release, "4.4.4")?;
    require_tool_version("DCM", &metrics_release, "1.40.0")?;
    Ok((dcl, dcm, linter_release, metrics_release))
}

fn report_paths(options: &Options, root: &Path) -> Result<(PathBuf, PathBuf)> {
    let published = root.join("website/src/data/benchmarks.json");
    let report = options
        .output
        .as_ref()
        .map_or_else(|| published.clone(), |path| root.join(path));
    if !options.corpus.is_empty()
        && (report == published
            || fs::canonicalize(&report)
                .ok()
                .zip(fs::canonicalize(&published).ok())
                .is_some_and(|(output, published)| output == published))
    {
        return Err("filtered runs must not overwrite the published benchmark report".into());
    }
    Ok((published, report))
}

fn machine_metadata() -> Result<Value> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let cpu = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .ok_or("CPU model missing")?;
    let memory = fs::read_to_string("/proc/meminfo")?;
    let memory_kb: f64 = memory
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or("memory missing")?
        .parse()?;
    Ok(
        json!({"cpu": cpu, "cores": std::thread::available_parallelism()?.get(),
        "memoryGb": memory_kb * 1024.0 / 1_000_000_000.0,
        "os": text(Command::new("uname").args(["-srm"]))?}),
    )
}

fn provenance(path: &Path) -> Result<Value> {
    let hash = text(Command::new("sha256sum").arg(path))?;
    Ok(json!({
        "path": path,
        "sha256": hash.split_whitespace().next().ok_or("binary hash")?,
    }))
}

pub fn run(options: &Options) -> Result<()> {
    if !options.corpus.is_empty() && options.output.is_none() {
        return Err("--corpus requires --output to protect the published benchmark report".into());
    }
    for id in &options.corpus {
        if !CORPORA.iter().any(|corpus| corpus.id == id) {
            return Err(format!("unknown corpus id: {id}").into());
        }
    }
    if !cfg!(target_os = "linux") {
        return Err("bench-compare requires Linux /proc and GNU time".into());
    }
    fs::read_to_string(format!("/proc/self/task/{}/children", std::process::id()))?;
    let root = std::env::current_dir()?;
    let cache = root.join("target/bench-corpora");
    fs::create_dir_all(&cache)?;
    let dart = dart_executable(options)?;
    let flutter = dart
        .ancestors()
        .nth(5)
        .ok_or("expected Flutter Dart SDK layout")?
        .join("bin/flutter");
    let hyperfine = match &options.hyperfine {
        Some(path) => fs::canonicalize(path)?,
        None => PathBuf::from(text(Command::new("mise").args(["which", "hyperfine"]))?),
    };
    let (dcl, dcm, linter_release, metrics_release) = competitor_tools(options, &dart)?;
    if !options.skip_build {
        output(Command::new("cargo").args(["build", "--locked", "--release", "--bin", "falcon"]))?;
    }
    let falcon = root.join("target/release/falcon");
    let dart_version = text(Command::new(&dart).arg("--version"))?;
    if !dart_version.contains("3.13.5") {
        return Err(format!("expected Dart 3.13.5, got {dart_version}").into());
    }
    let falcon_version = text(Command::new(&falcon).arg("version"))?;
    let binary_hash = text(Command::new("sha256sum").arg(&falcon))?
        .split_whitespace()
        .next()
        .ok_or("binary hash")?
        .to_owned();
    let (published_path, report_path) = report_paths(options, &root)?;
    let previous: Option<Value> = fs::read(&published_path)
        .ok()
        .map(|bytes| serde_json::from_slice(&bytes))
        .transpose()?;
    let (config, names) = configuration();
    let (dcl_config, dcl_names) = configuration_for(true);
    let mut corpora = Vec::with_capacity(CORPORA.len());
    let mut results = Vec::with_capacity(CORPORA.len() * 10);
    let benchmark = BenchmarkContext {
        root: &root,
        cache: &cache,
        dart: &dart,
        flutter: &flutter,
        hyperfine: &hyperfine,
        falcon: &falcon,
        options,
        config: &config,
        names: &names,
        dcl_config: &dcl_config,
        dcl_names: &dcl_names,
        dcl: &dcl,
        dcm: &dcm,
    };
    for corpus in CORPORA {
        if !options.corpus.is_empty() && !options.corpus.iter().any(|id| id == corpus.id) {
            continue;
        }
        corpora.push(benchmark_corpus(
            &benchmark,
            corpus,
            previous.as_ref(),
            &mut results,
        )?);
    }
    let report = json!({"generatedAt": text(Command::new("date").args(["-u", "+%Y-%m-%dT%H:%M:%SZ"]))?,
        "machine": machine_metadata()?,
        "tools": [
            {"id": "falcon", "name": "falcon", "version": falcon_version, "binarySha256": binary_hash, "provenance": provenance(&falcon)?},
            {"id": "dart-analyze", "name": "dart analyze", "version": "3.13.5", "provenance": provenance(&dart)?},
            {"id": "dcl", "name": "Dart Code Linter", "version": linter_release, "package": "dart_code_linter", "homepage": "https://pub.dev/packages/dart_code_linter", "provenance": provenance(&dcl)?},
            {"id": "dcm", "name": "DCM", "version": metrics_release, "homepage": "https://dcm.dev", "provenance": provenance(&dcm)?, "reporter": "console", "licenseRequirement": "activated local license; machine reporters are not required"}
        ],
        "harness": {"hyperfine": text(Command::new(&hyperfine).arg("--version"))?, "warmups": options.warmups, "coldRuns": options.cold_runs, "warmRuns": options.runs, "rssLimitMb": f64::from(MAX_RSS_KB) * 1024.0 / 1_000_000.0, "rules": names, "dclRules": dcl_names, "excludedDclCommands": DCL_COMMANDS, "excludedDclRules": excluded_dcl_rules(), "dclEligibilityPolicy": "Pinned DCM 1.40.0 Free eligibility; exclusions apply equally to all three tools; remaining ids strictly validated in preflight", "corpusFilter": options.corpus, "analyzerState": "ANALYZER_STATE_LOCATION_OVERRIDE isolated per corpus, rule family and tool; removed before cold invocations", "dcmEnvironment": "CI unset to use the activated local license; activation state retained"},
        "dclFileExclusions": {"patterns": DCL_FILE_EXCLUSIONS, "appliesTo": ["falcon", "dcl", "dcm"], "scenarios": ["dcl-rules-cold", "dcl-rules-warm"], "reason": "Common generated-file exclusion union; DCM intrinsically suppresses generated-file diagnostics", "corpusCounts": "Published corpus file and line counts describe the full same-rules corpus; DCL scenarios use the subset excluding these generated files"},
        "corpora": corpora,
        "scenarios": [
            {"id": "same-rules-cold", "title": "Same rules, cold analyzer cache", "description": "Identical enabled lint names; isolated analyzer state removed before each invocation; OS file cache remains warm.", "comparator": "dart-analyze", "tools": ["falcon", "dart-analyze"]},
            {"id": "same-rules-warm", "title": "Same rules, warm analyzer cache", "description": "Identical enabled lint names; analyzer state retained after warmups; each sample starts a new process.", "comparator": "dart-analyze", "tools": ["falcon", "dart-analyze"]},
            {"id": "dcl-rules-cold", "title": "DCL rules, cold analyzer cache", "description": "Identical upstream DCL lint ids enabled in falcon, DCL and DCM; analyzer state removed before each invocation; OS file cache remains warm.", "comparator": "dcl", "tools": ["falcon", "dcl", "dcm"]},
            {"id": "dcl-rules-warm", "title": "DCL rules, warm analyzer cache", "description": "Identical upstream DCL lint ids enabled in falcon, DCL and DCM; analyzer state retained after warmups; each sample starts a new process.", "comparator": "dcl", "tools": ["falcon", "dcl", "dcm"]}
        ],
        "results": results});
    write_json(&report_path, &report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Complete console capture from DCM 1.40.0 against the pinned bloc corpus.
    const DCM_CONSOLE_SAMPLE: &str = r"⠙ Analyzing...⠹ Analyzing 8 files... 3ms⠸ Analyzing 8 files... 0.1s✔ Analysis is completed. Preparing the results: 0.1s

packages/bloc/lib/src/bloc.dart (9 issues):
WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:61:27
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:64:20
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:64:61
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:68:46
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:70:31
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:191:39
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:197:8
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:326:23
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc.dart:342:49
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic


packages/bloc/lib/src/bloc_observer.dart (26 issues):
WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:17:26
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:23:21
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:23:30
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:30:26
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:30:48
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:40:10
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:40:19
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:41:16
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:41:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:49:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:57:10
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:57:19
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:69:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:98:26
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:105:21
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:105:30
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:112:26
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:112:48
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:120:10
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:120:19
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:121:16
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:121:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:129:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:137:10
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:137:19
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic

WARNING   Avoid 'dynamic' types. Try replacing it with a different type.
          at packages/bloc/lib/src/bloc_observer.dart:148:25
          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic


Scanned files: 8
✖ total lint issues - 35 (warning issues: 35)

To get a single-line per-violation format, pass the '--concise' flag (or just '-c')
";

    #[test]
    fn parses_dcm_console_rule_links_and_checks_summary() {
        let issue = "WARNING   Avoid 'dynamic' types. Try replacing it with a different type.\n          at packages/bloc/lib/src/bloc.dart:61:27\n          avoid-dynamic : https://dcm.dev/docs/rules/common/avoid-dynamic\n";
        let sample = DCM_CONSOLE_SAMPLE;
        let (total, counts) = diagnostics("dcm", sample.as_bytes()).unwrap();
        assert_eq!(total, 35);
        assert_eq!(counts["avoid-dynamic"], 35);
        assert!(
            diagnostics(
                "dcm",
                sample.replace("issues - 35", "issues - 36").as_bytes()
            )
            .is_err()
        );
        assert!(diagnostics("dcm", issue.as_bytes()).is_err());
        assert_eq!(
            diagnostics("dcm", "✔ no issues found!".as_bytes())
                .unwrap()
                .0,
            0
        );
        assert!(diagnostics("dcm", "✔ no issues found!\nConfig errors:\nanalysis_options.yaml (2 issues):\nWARNING   'fictional-rule' is not recognized as a valid rule name.\n          unknown-rule : https://dcm.dev/docs/rules/\n".as_bytes()).is_err());
        assert!(diagnostics("dcm", b"Activation required").is_err());
    }

    #[test]
    fn parses_dcl_versioned_reports_and_configuration_issues() {
        let (count, rules) = diagnostics("dcl", br#"progress
{"formatVersion":2,"records":[{"path":"lib/a.dart","issues":[{"ruleId":"avoid-dynamic","severity":"warning"},{"ruleId":"unknown-config","severity":"warning"}],"antiPatternCases":[]}]}"#).unwrap();
        assert_eq!(count, 2);
        assert_eq!(rules["avoid-dynamic"], 1);
        assert_eq!(rules["unknown-config"], 1);
        assert!(diagnostics("dcl", b"{}").is_err());
        assert!(diagnostics("dcl", br#"{"formatVersion":2,"records":[{}]}"#).is_err());
        assert!(diagnostics("dcl", br#"{"formatVersion":2,"records":[{"issues":[{}]}]}"#).is_err());
    }

    #[test]
    fn filtered_runs_require_explicit_output() {
        #[derive(clap::Parser)]
        struct Cli {
            #[command(flatten)]
            options: Options,
        }
        use clap::Parser;
        assert!(Cli::try_parse_from(["bench", "--corpus", "flutter"]).is_err());
        let cli = Cli::try_parse_from([
            "bench",
            "--corpus",
            "flutter,dart-sdk",
            "--output",
            "/tmp/benchmark.json",
            "--dcl",
            "/tmp/dcl.snapshot",
            "--dcm",
            "/tmp/dcm",
        ])
        .unwrap();
        assert_eq!(cli.options.corpus, ["flutter", "dart-sdk"]);
        assert_eq!(
            cli.options.output,
            Some(PathBuf::from("/tmp/benchmark.json"))
        );
        assert_eq!(cli.options.dcl, Some(PathBuf::from("/tmp/dcl.snapshot")));
        assert_eq!(cli.options.dcm, Some(PathBuf::from("/tmp/dcm")));
    }

    #[test]
    fn dcl_config_enables_only_matching_lints_and_maps_upstream_ids() {
        let (config, names) = configuration_for(true);
        assert_eq!(names.len(), 31);
        assert!(DCL_COMMANDS.iter().all(|name| !names.contains(name)));
        assert!(DCM_EXCLUSIONS.iter().all(|(id, _)| !names.contains(id)));
        for rule in RULE_METADATA {
            let section = if matches!(rule.kind, RuleKind::CrossFile { .. }) {
                "cross-file"
            } else {
                "linter"
            };
            let enabled = config[section]["rules"][rule.group.as_str()][rule.name] == "warn";
            let expected = matches!(rule.source, RuleSource::DartCodeLinter(name)
                if !DCL_COMMANDS.contains(&name)
                    && !DCM_EXCLUSIONS.iter().any(|(id, _)| *id == name));
            assert_eq!(enabled, expected, "{}", rule.name);
            if let RuleSource::DartCodeLinter(name) = rule.source
                && expected
            {
                let counts = BTreeMap::from([(rule.name.to_owned(), 1)]);
                let (mapped, total) = comparable_counts("falcon", &counts, &names);
                assert_eq!(mapped[name], 1);
                assert_eq!(total, 1);
            }
        }
    }

    #[test]
    fn config_enables_only_official_sources() {
        let (config, names) = configuration();
        assert_ne!(names, [] as [&str; 0]);
        assert_eq!(config["linter"]["rules"]["recommended"], false);
        assert_eq!(config["cross-file"]["rules"]["recommended"], false);
        let enabled = ["linter", "cross-file"]
            .iter()
            .map(|section| {
                config[section]["rules"]
                    .as_object()
                    .unwrap()
                    .values()
                    .filter_map(Value::as_object)
                    .map(serde_json::Map::len)
                    .sum::<usize>()
            })
            .sum::<usize>();
        assert_eq!(enabled, names.len());
    }

    #[test]
    fn parses_analyzer_machine_records_only() {
        let (count, rules) = diagnostics(
            "dart-analyze",
            b"Analyzing...\nINFO|LINT|AVOID_PRINT|/tmp/a.dart|1|1|5|print\n",
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(rules["avoid_print"], 1);
    }

    #[test]
    fn parses_falcon_diagnostics_and_rejects_bad_exports() {
        let (count, rules) = diagnostics(
            "falcon",
            br#"[{"rule":"avoid-print"},{"rule":"avoid-print"}]"#,
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(rules["avoid-print"], 2);
        assert!(diagnostics("falcon", b"{}").is_err());
        assert!(diagnostics("falcon", br"[{}]").is_err());
    }

    #[test]
    fn separates_mapped_lints_from_configuration_diagnostics() {
        let (_, names) = configuration();
        let (_, falcon) = diagnostics(
            "falcon",
            br#"[{"rule":"prefer-iterable-where-type"},{"rule":"syntax-error"}]"#,
        )
        .unwrap();
        let (_, dart) = diagnostics("dart-analyze", b"INFO|LINT|PREFER_ITERABLE_WHERETYPE|a.dart|1|1|1|lint\nWARNING|STATIC_WARNING|INCOMPATIBLE_LINT|analysis_options.yaml|1|1|1|configuration\n").unwrap();
        let (falcon, falcon_lints) = comparable_counts("falcon", &falcon, &names);
        let (dart, dart_lints) = comparable_counts("dart-analyze", &dart, &names);
        assert_eq!(falcon_lints, 1);
        assert_eq!(dart_lints, 1);
        assert_eq!(
            falcon["prefer_iterable_wheretype"],
            dart["prefer_iterable_wheretype"]
        );
        assert_eq!(dart["incompatible_lint"], 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn guard_captures_output_and_preserves_diagnostic_exit_status() {
        let directory = tempfile::tempdir().unwrap();
        let output = guarded_output(
            Command::new("sh").args(["-c", "printf diagnostics; printf warning >&2; exit 3"]),
            &directory.path().join("capture"),
            u64::from(MAX_RSS_KB),
        )
        .unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout, b"diagnostics");
        assert_eq!(output.stderr, b"warning");
        assert!(resident_kb(std::process::id()).unwrap() > 0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn guard_terminates_processes_over_its_memory_limit() {
        let directory = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let error = guarded_output(
            Command::new("sh").args(["-c", "sleep 30 & wait"]),
            &directory.path().join("capture"),
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("resident-memory safety limit"));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn tree_termination_also_kills_a_live_grandchild() {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("grandchild");
        let mut child = Command::new("sh")
            .args(["-c", "sleep 30 & echo $! > \"$1\"; wait", "--"])
            .arg(&pid_file)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let descendant = loop {
            if let Ok(contents) = fs::read_to_string(&pid_file)
                && let Ok(pid) = contents.trim().parse::<u32>()
            {
                break pid;
            }
            if Instant::now() > deadline {
                kill_tree(child.id()).unwrap();
                child.wait().unwrap();
                panic!("grandchild did not become ready");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        kill_tree(child.id()).unwrap();
        child.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match fs::read_to_string(format!("/proc/{descendant}/status")) {
                Ok(status) if status.lines().any(|line| line.starts_with("State:\tZ")) => break,
                Err(error) if process_gone(&error) => break,
                Err(error) => panic!("{error}"),
                Ok(_) => {}
            }
            assert!(Instant::now() < deadline, "grandchild survived termination");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn finds_children_spawned_by_nonleader_threads() {
        let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
        let (release, done) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let mut child = Command::new("sleep").arg("30").spawn().unwrap();
            pid_sender.send(child.id()).unwrap();
            let _ = done.recv();
            child.kill().unwrap();
            child.wait().unwrap();
        });
        let child = pid_receiver.recv().unwrap();
        let mut found = false;
        visit_children(std::process::id(), |pid| {
            found |= pid == child;
            Ok(())
        })
        .unwrap();
        drop(release);
        thread.join().unwrap();
        assert!(found);
        assert!(process_gone(&std::io::Error::from_raw_os_error(3)));
    }

    #[test]
    fn exclusions_preserve_normal_relative_paths_and_match_source_counts() {
        let excluded: Vec<_> = EXCLUDED.iter().map(|path| (*path).to_owned()).collect();
        let filter = serde_json::from_value(json!(falcon_includes(&excluded))).unwrap();
        assert_eq!(
            falcon_project::walk_project(&[], &filter, false)
                .unwrap()
                .files,
            [] as [(std::path::PathBuf, std::string::String); 0]
        );
        assert!(filter.admits("./lib/a.dart"));
        assert!(!filter.admits("./pkg/.agents/a.dart"));
        assert!(!filter.admits("./.agents/a.dart"));
        assert!(!filter.admits("./.hidden.dart"));
        assert!(!filter.admits("./build/a.dart"));
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(".agents")).unwrap();
        fs::write(directory.path().join(".agents/a.dart"), "").unwrap();
        fs::write(directory.path().join(".hidden.dart"), "").unwrap();
        fs::write(directory.path().join("a.dart"), "").unwrap();
        assert_eq!(
            entries(directory.path())
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "dart"))
                .count(),
            1
        );
    }

    #[test]
    fn records_corpus_relative_commands_without_sdk_install_paths() {
        let project = Path::new("/machine/cache/corpus/.");
        let args = vec![
            "check".to_owned(),
            ".".to_owned(),
            "--config".to_owned(),
            project.join("falcon.benchmark.json").display().to_string(),
        ];
        assert_eq!(
            recorded_command(
                Path::new("/machine/repo/target/release/falcon"),
                &args,
                project
            ),
            "'falcon' 'check' '.' '--config' 'falcon.benchmark.json'"
        );
        assert_eq!(
            recorded_command(
                Path::new("/machine/sdk/bin/dart"),
                &["analyze".to_owned()],
                project
            ),
            "'dart' 'analyze'"
        );
    }

    #[test]
    fn checkout_recovers_after_failed_fetch_and_updates_pinned_revision() {
        let directory = tempfile::tempdir().unwrap();
        let repository = directory.path().join("repository");
        let checkout = directory.path().join("checkout");
        output(Command::new("git").arg("init").arg(&repository)).unwrap();
        let git = |path: &Path, args: &[&str]| {
            text(Command::new("git").arg("-C").arg(path).args(args)).unwrap()
        };
        let mut revisions = Vec::new();
        for message in ["first", "second"] {
            git(
                &repository,
                &[
                    "-c",
                    "user.name=benchmark-test",
                    "-c",
                    "user.email=benchmark@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "--allow-empty",
                    "-m",
                    message,
                ],
            );
            revisions.push(git(&repository, &["rev-parse", "HEAD"]));
        }
        let repository = repository.to_str().unwrap();
        assert!(prepare_checkout(&checkout, repository, "missing-revision").is_err());
        for revision in revisions {
            prepare_checkout(&checkout, repository, &revision).unwrap();
            assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), revision);
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn guarded_commands_inherit_the_foreground_process_group() {
        let directory = tempfile::tempdir().unwrap();
        let captured = guarded_output(
            Command::new("sh").args(["-c", "cat /proc/$$/stat"]),
            &directory.path().join("capture"),
            u64::from(MAX_RSS_KB),
        )
        .unwrap();
        let group = |stat: &str| {
            stat.rsplit_once(") ")
                .unwrap()
                .1
                .split_whitespace()
                .nth(2)
                .unwrap()
                .parse::<u32>()
                .unwrap()
        };
        assert_eq!(
            group(&String::from_utf8(captured.stdout).unwrap()),
            group(&fs::read_to_string("/proc/self/stat").unwrap()),
        );
    }

    #[test]
    fn quotes_shell_paths() {
        assert_eq!(quote(Path::new("it's here")), "'it'\\''s here'");
    }
}
