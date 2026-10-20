use clap::{Args, Parser, Subcommand};
use falcon_rules::meta::{Domain, RULE_METADATA, RuleGroup, RuleMeta, RuleSource};
use rayon::prelude::*;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod bench_compare;

#[path = "../../crates/falcon_rules/tests/support/corpus.rs"]
mod corpus;
use corpus::{Expectation, cross_file_project_roots, parse_expectations};

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Generate rule implementation and fixture stubs.
    Codegen {
        #[command(subcommand)]
        target: Codegen,
    },
    /// Validate rule implementations against the golden corpus.
    ValidateRules(ValidateRules),
    /// Enforce the performance budget on the jfit mobile library.
    PerfLock(PerfLock),
    /// Compare release falcon with dart analyze on pinned public projects.
    BenchCompare(bench_compare::Options),
    /// Generate the configuration schema.
    Schema {
        #[arg(long)]
        check: bool,
    },
    /// Generate the documentation website's data.
    Docgen,
    /// Set the workspace version and every pinned reference to it.
    Version {
        /// The new version, e.g. 0.4.2.
        version: String,
    },
}

#[derive(Subcommand)]
enum Codegen {
    Rule {
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(RuleGroup::ALL.iter().map(|group| group.as_str())))]
        group: String,
        #[arg(long, help = "Rule id, used verbatim")]
        name: String,
    },
}

#[derive(Args)]
struct ValidateRules {
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
    #[arg(long, help = "Falcon binary (default: build target/debug/falcon)")]
    falcon_bin: Option<PathBuf>,
    #[arg(long, help = "Output results as JSON")]
    json: bool,
}

#[derive(Args)]
struct PerfLock {
    #[arg(
        long,
        help = "Corpus (default: apps/mobile/lib under required $JFIT_PATH)"
    )]
    corpus: Option<PathBuf>,
    #[arg(long, default_value_t = 5, help = "Timed runs to take the median of")]
    runs: usize,
    #[arg(
        long,
        default_value_t = 1000,
        help = "Wall-clock budget in milliseconds"
    )]
    budget_ms: u128,
    #[arg(long, help = "Falcon binary (default: target/release/falcon)")]
    falcon_bin: Option<PathBuf>,
    #[arg(long, help = "Don't rebuild the release binary first")]
    skip_build: bool,
    #[arg(long, help = "Output results as JSON")]
    json: bool,
}

fn main() {
    match Cli::parse().task {
        Task::Codegen {
            target: Codegen::Rule { group, name },
        } => codegen(group, name),
        Task::ValidateRules(options) => validate_rules(options),
        Task::PerfLock(options) => perf_lock(options),
        Task::BenchCompare(options) => {
            if let Err(error) = bench_compare::run(&options) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Task::Schema { check } => schema(check),
        Task::Docgen => docgen(),
        Task::Version { version } => set_version(&version),
    }
}

/// Generate `schema/falcon.schema.json` from the rule metadata, or with
/// `--check` verify the committed file matches (CI drift guard).
fn schema(check: bool) {
    let path = Path::new("schema/falcon.schema.json");
    let generated = falcon_rules::schema::config_schema_string();

    if check {
        let current = fs::read_to_string(path).unwrap_or_default();
        if current == generated {
            println!("schema up to date: {}", path.display());
        } else {
            eprintln!(
                "error: {} is out of date; run `cargo xtask schema`",
                path.display()
            );
            std::process::exit(1);
        }
        return;
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create schema dir");
    }
    fs::write(path, &generated).expect("write schema");
    println!("wrote {}", path.display());
}

/// Docs, CLI data, and launcher sources: every file that pins the falcon version.
fn versioned_files(root: &Path) -> Vec<PathBuf> {
    let changelog = root.join("CHANGELOG.md");
    let mut files: Vec<PathBuf> = [root.to_path_buf(), root.join("docs")]
        .iter()
        .flat_map(|dir| fs::read_dir(dir).expect("read docs dir"))
        .map(|entry| entry.expect("read docs entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .filter(|path| *path != changelog)
        .collect();
    files.extend([
        root.join("website/src/data/cli.json"),
        root.join("packages/falcon/pubspec.yaml"),
        root.join("packages/falcon/lib/src/version.dart"),
    ]);
    files
}

/// `text` with every pinned falcon version (release tag, archive name, mise pin,
/// cli.json `version`, launcher YAML version and Dart const) replaced by `version`.
fn with_version_references(text: &str, version: &str) -> String {
    Regex::new(
        r#"(?m)(releases/download/v|falcon-|dart_falcon" = \{ version = "|"version": "|^version: |^const version = ')\d+\.\d+\.\d+"#,
    )
    .unwrap()
    .replace_all(text, format!("${{1}}{version}"))
    .into_owned()
}

/// `cargo xtask version 0.4.2`: bump `[workspace.package]`, every pinned
/// reference in `versioned_files`, and the workspace entries in Cargo.lock.
fn set_version(version: &str) {
    if !Regex::new(r"^\d+\.\d+\.\d+$").unwrap().is_match(version) {
        eprintln!("error: expected a version like 0.4.2, got `{version}`");
        std::process::exit(1);
    }
    let root = workspace_root();
    let manifest_path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read Cargo.toml");
    let workspace_version =
        Regex::new(r#"(\[workspace\.package\]\nversion\s*=\s*")[^"]+"#).unwrap();
    if !workspace_version.is_match(&manifest) {
        eprintln!("error: Cargo.toml has no `version` directly under [workspace.package]");
        std::process::exit(1);
    }
    let manifest = workspace_version.replace(&manifest, format!("${{1}}{version}"));
    fs::write(&manifest_path, manifest.as_ref()).expect("write Cargo.toml");
    println!("updated {}", manifest_path.display());

    for path in versioned_files(&root) {
        let text = fs::read_to_string(&path).expect("read versioned file");
        let updated = with_version_references(&text, version);
        if updated != text {
            fs::write(&path, updated).expect("write versioned file");
            println!("updated {}", path.display());
        }
    }

    let status = Command::new("cargo")
        .args(["update", "--workspace", "--offline"])
        .current_dir(&root)
        .status()
        .expect("failed to spawn cargo update");
    if !status.success() {
        eprintln!("error: cargo update --workspace failed");
        std::process::exit(1);
    }
}

/// Convert `snake_case` to `PascalCase` for the rule struct name.
fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Render the Rule impl stub for a new rule.
fn rule_stub(struct_name: &str) -> String {
    format!(
        r#"use falcon_analyze::{{AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal}};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{{Expr, Program}};

#[derive(Default)]
pub struct {struct_name};

impl Rule for {struct_name} {{
    fn prepare<'a>(&self, program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {{
        let _ = (program, ctx);
        RulePreparation::Structural(Box::new(Collector {{ diagnostics: Vec::new() }}))
    }}
}}

struct Collector {{
    diagnostics: Vec<Diagnostic>,
}}

falcon_analyze::structural_visitor! {{
impl<'a> StructuralRuleVisitor<'a> for Collector {{
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {{
        let _ = node;
        StructuralTraversal::Continue
    }}

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {{
        self.diagnostics
    }}
}}
}}
"#
    )
}

/// Generate a rule implementation stub plus corpus fixture skeletons.
///
/// `cargo xtask codegen rule --group suspicious --name avoid-foo`
///
/// The rule id is taken from `--name` verbatim (no kebab/snake derivation); the
/// module file is `lint/<group>/<name-with-dashes-as-underscores>.rs`.
fn codegen(group: String, rule_id: String) {
    // The module file name is the rule id with dashes normalized to underscores.
    let module = rule_id.replace('-', "_");

    let root = workspace_root();
    let rule_file = root.join(format!(
        "crates/falcon_rules/src/lint/{}/{}.rs",
        group, module
    ));
    let corpus_dir = root.join(format!("crates/falcon_rules/tests/corpus/{}", rule_id));

    if rule_file.exists() {
        eprintln!("error: {} already exists", rule_file.display());
        std::process::exit(1);
    }

    let struct_name = pascal_case(&module);
    fs::create_dir_all(rule_file.parent().expect("rule directory")).expect("create rule directory");
    fs::write(&rule_file, rule_stub(&struct_name)).expect("write rule stub");

    fs::create_dir_all(&corpus_dir).expect("create corpus dir");
    let bad = corpus_dir.join("bad.dart");
    let good = corpus_dir.join("good.dart");
    if !bad.exists() {
        fs::write(
            &bad,
            format!(
                "// TODO: violations annotated as /* expect: {} */\n",
                rule_id
            ),
        )
        .expect("write bad.dart");
    }
    if !good.exists() {
        fs::write(&good, "// TODO: compliant code, no annotations\n").expect("write good.dart");
    }

    println!("generated:");
    println!("  {}", rule_file.display());
    println!("  {}", bad.display());
    println!("  {}", good.display());
    println!();
    println!("next steps:");
    let group_module = root.join(format!("crates/falcon_rules/src/lint/{group}.rs"));
    if !group_module.exists() {
        println!(
            "  create crates/falcon_rules/src/lint/{group}.rs and add `pub mod {group};` to crates/falcon_rules/src/lint.rs"
        );
    }
    println!(
        "  1. add `pub mod {};` to crates/falcon_rules/src/lint/{}.rs",
        module, group
    );
    let category = RuleGroup::ALL
        .iter()
        .find(|category| category.as_str() == group)
        .unwrap();
    println!(
        "  2. add RuleMeta::file::<lint::{group}::{module}::{struct_name}>(\
         \"{rule_id}\", RuleGroup::{category:?}, FLUTTER /* or NONE */, recommended, source) to \
         RULE_METADATA (crates/falcon_rules/src/meta.rs); use resolved for semantic rules"
    );
    println!("  3. add `{rule_id}` under linter.rules.{group} in the root falcon.json showcase");
    println!(
        "  4. implement the rule, fill in fixtures, then run `cargo xtask validate-rules --rule {}`",
        rule_id
    );
}

fn workspace_root() -> PathBuf {
    // When run via `cargo xtask`, CWD is the workspace root.
    std::env::current_dir().unwrap()
}

#[derive(Debug, serde::Deserialize)]
struct DiagnosticJson {
    rule: String,
    message: String,
    line: usize,
    col: usize,
    #[serde(default)]
    file_path: String,
}

fn character_column(source: &str, line: usize, utf16_column: usize) -> usize {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let mut units = 0;
    let prefix_bytes: usize = text
        .chars()
        .take_while(|character| {
            units += character.len_utf16();
            units < utf16_column
        })
        .map(char::len_utf8)
        .sum();
    let column = expectation_segments(text)
        .map(|(start, segment)| {
            let end = prefix_bytes.saturating_sub(start).min(segment.len());
            segment[..end].chars().count()
        })
        .sum::<usize>();
    let trimmed_length = expectation_segments(text)
        .flat_map(|(_, segment)| segment.chars())
        .enumerate()
        .filter_map(|(index, character)| (!character.is_whitespace()).then_some(index + 1))
        .last()
        .unwrap_or(0);
    1 + column.min(trimmed_length)
}

#[path = "../../crates/falcon_rules/tests/support/sdk_fixture.rs"]
mod sdk_fixture;

/// Bundled listed corpus fixtures always use parsed dependency sources,
/// independently of the developer machine's installed SDK.
struct FalconRunner {
    binary: String,
    fallback_sdk: Option<PathBuf>,
}

impl FalconRunner {
    fn new(binary: PathBuf, workspace: &Path, corpus: &Path) -> Self {
        let fallback_sdk = corpus_fixture_sdk(workspace, corpus);
        Self {
            binary: binary.to_string_lossy().into_owned(),
            fallback_sdk,
        }
    }

    fn sdk_for(&self, file: &Path) -> Option<&Path> {
        self.fallback_sdk
            .as_deref()
            .filter(|_| sdk_fixture::requires_sdk(file))
    }

    fn run(&self, file: &Path, config: Option<&Path>) -> Result<Vec<DiagnosticJson>, String> {
        // Stage the parsed SDK, dependency sources and package metadata together,
        // using the same fixture builder as the in-process corpus harness.
        let staged = self.sdk_for(file).map(|_| {
            let directory = sdk_fixture::source_directory(file);
            (sdk_fixture::stage_project(directory), directory)
        });
        let analyzed_file = staged.as_ref().map_or_else(
            || file.to_path_buf(),
            |(project, directory)| sdk_fixture::staged_file(project.path(), directory, file),
        );
        let mut cmd = Command::new(&self.binary);
        cmd.args(["check", "--format", "json"]);
        cmd.arg(&analyzed_file);
        if let Some(config_path) = config {
            cmd.args(["--config", config_path.to_str().unwrap_or("")]);
        }
        if let Some((project, _)) = &staged {
            cmd.env(
                "DART_SDK",
                project.path().join("flutter/bin/cache/dart-sdk"),
            );
        }
        let output = cmd
            .env("RUST_LOG", "error")
            .output()
            .map_err(|e| format!("Failed to run falcon binary '{}': {}", self.binary, e))?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(format!(
                "falcon exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        let mut diagnostics =
            serde_json::from_str::<Vec<DiagnosticJson>>(trimmed).map_err(|e| {
                format!(
                    "JSON parse error: {}\nOutput was: {}\nStderr: {}",
                    e,
                    &trimmed[..trimmed.len().min(200)],
                    String::from_utf8_lossy(&output.stderr)
                )
            })?;
        if let Some((project, directory)) = &staged {
            let staged_root = falcon_analyze::canonical_or_lexical(&sdk_fixture::staged_directory(
                project.path(),
                directory,
            ));
            for diagnostic in &mut diagnostics {
                if let Ok(relative) = Path::new(&diagnostic.file_path).strip_prefix(&staged_root) {
                    diagnostic.file_path = directory.join(relative).to_string_lossy().into_owned();
                }
            }
        }
        Ok(diagnostics)
    }

    fn run_or_exit(&self, file: &Path, config: Option<&Path>) -> Vec<DiagnosticJson> {
        self.run(file, config).unwrap_or_else(|error| {
            eprintln!(
                "error: falcon invocation failed for {}: {error}",
                file.display()
            );
            std::process::exit(1);
        })
    }
}

fn corpus_fixture_sdk(workspace: &Path, corpus: &Path) -> Option<PathBuf> {
    let bundled = workspace.join("crates/falcon_rules/tests/corpus");
    (falcon_analyze::canonical_or_lexical(corpus) == falcon_analyze::canonical_or_lexical(&bundled))
        .then(|| workspace.join("crates/falcon_rules/tests/support/sdk"))
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

fn recursive_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("cannot walk {}: {error}", root.display()))
        })
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect();
    files.sort_unstable();
    files
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

fn build_falcon(release: bool) -> PathBuf {
    let workspace = workspace_root();
    eprintln!(
        "{}",
        if release {
            "building release falcon binary..."
        } else {
            "building falcon binary (debug)..."
        }
    );
    let mut command = Command::new("cargo");
    command.args(["build", "--bin", "falcon"]);
    if release {
        command.arg("--release");
    }
    let status = command
        .current_dir(&workspace)
        .status()
        .expect("failed to spawn cargo build");
    if !status.success() {
        eprintln!(
            "{}",
            if release {
                "error: release build failed"
            } else {
                "error: falcon build failed"
            }
        );
        std::process::exit(1);
    }
    workspace.join(if release {
        "target/release/falcon"
    } else {
        "target/debug/falcon"
    })
}

fn validate_rules(options: ValidateRules) {
    let workspace = workspace_root();
    let ValidateRules {
        corpus: corpus_path,
        threshold,
        rule: rule_filter,
        falcon_bin,
        json: json_output,
    } = options;
    let corpus_path =
        corpus_path.unwrap_or_else(|| workspace.join("crates/falcon_rules/tests/corpus"));
    let falcon_bin = falcon_bin.unwrap_or_else(|| build_falcon(false));
    let falcon = FalconRunner::new(falcon_bin, &workspace, &corpus_path);

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

    // Each rule spawns one falcon process per fixture; rules are independent.
    let outcomes: Vec<RuleOutcome> = rule_dirs
        .par_iter()
        .map(|rule_dir| validate_rule_dir(rule_dir, &workspace, &corpus_path, &falcon, threshold))
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
        std::process::exit(1);
    }
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
    workspace: &Path,
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
    let rule_config_path = rule_dir.join("config.json");
    let uses_generated_config = !rule_config_path.exists();
    let generated_config_path = workspace.join(format!(
        "target/validate-rules-{}-{rule_name}.json",
        std::process::id()
    ));
    let rule_config = if uses_generated_config {
        falcon_rules::meta::RULE_METADATA
            .iter()
            .find(|meta| meta.name == rule_name)
            .and_then(|meta| docgen_config_for(corpus_path, meta, &generated_config_path))
    } else {
        Some(rule_config_path)
    };

    let mut outcome = RuleOutcome::default();
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
    if uses_generated_config {
        let _ = fs::remove_file(&generated_config_path);
    }
    outcome
}

fn perf_corpus(corpus: Option<PathBuf>, root: Option<std::ffi::OsString>) -> Option<PathBuf> {
    corpus.or_else(|| root.map(|root| PathBuf::from(root).join("apps/mobile/lib")))
}

fn perf_lock(options: PerfLock) {
    let PerfLock {
        corpus,
        runs,
        budget_ms,
        falcon_bin,
        skip_build,
        json: json_output,
    } = options;
    let falcon_bin = falcon_bin.unwrap_or_else(|| workspace_root().join("target/release/falcon"));
    let runs = runs.max(1);

    let corpus = perf_corpus(corpus, std::env::var_os("JFIT_PATH")).unwrap_or_else(|| {
        eprintln!("error: set JFIT_PATH or pass --corpus");
        std::process::exit(1);
    });
    if !corpus.exists() {
        eprintln!("error: corpus path does not exist: {}", corpus.display());
        std::process::exit(1);
    }

    if !skip_build {
        build_falcon(true);
    }
    if !falcon_bin.exists() {
        eprintln!("error: falcon binary not found: {}", falcon_bin.display());
        std::process::exit(1);
    }

    // Warm-up run primes the OS file cache so timed runs measure the linter,
    // not cold disk I/O.
    let run_once = |label: &str| -> u128 {
        let start = std::time::Instant::now();
        let output = Command::new(&falcon_bin)
            // --exit-code 0 keeps "violations found" from reading as failure;
            // a non-zero exit therefore means the pipeline itself broke.
            .args(["check", "--quiet", "--exit-code", "0"])
            .arg(&corpus)
            .output()
            .expect("failed to spawn falcon");
        let elapsed = start.elapsed().as_millis();
        if !output.status.success() {
            eprintln!(
                "error: falcon check failed ({}): {}",
                label,
                String::from_utf8_lossy(&output.stderr)
            );
            std::process::exit(1);
        }
        elapsed
    };

    run_once("warm-up");
    let mut times: Vec<u128> = (0..runs)
        .map(|n| {
            let t = run_once(&format!("run {}", n + 1));
            eprintln!("  run {}: {}ms", n + 1, t);
            t
        })
        .collect();
    times.sort_unstable();
    let median = times[times.len() / 2];
    let min = times[0];
    let max = times[times.len() - 1];
    let pass = median < budget_ms;

    if json_output {
        let report = serde_json::json!({
            "corpus": corpus.display().to_string(),
            "runs": runs,
            "budget_ms": budget_ms,
            "min_ms": min,
            "median_ms": median,
            "max_ms": max,
            "pass": pass,
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!();
        println!("── perf-lock ───────────────────────────────────────────");
        println!("  Corpus:         {}", corpus.display());
        println!("  Runs:           {runs}");
        println!("  Budget:         {budget_ms}ms");
        println!("  Min:            {min}ms");
        println!("  Median:         {median}ms");
        println!("  Max:            {max}ms");
        println!("  Status:         {}", if pass { "PASS" } else { "FAIL" });
        println!("────────────────────────────────────────────────────────");
    }

    if !pass {
        std::process::exit(1);
    }
}

/// Map a rule's snake_case module name from its kebab id.
fn snake_of(name: &str) -> String {
    name.replace('-', "_")
}

/// The source file path for a rule's implementation.
fn rule_source_path(root: &Path, meta: &RuleMeta) -> PathBuf {
    let snake = snake_of(meta.name);
    if meta.is_cross_file() {
        root.join(format!("crates/falcon_rules/src/cross_file/{snake}.rs"))
    } else {
        root.join(format!(
            "crates/falcon_rules/src/lint/{}/{}.rs",
            meta.group.as_str(),
            snake
        ))
    }
}

const PROVENANCE: [&str; 4] = ["Ported from", "Port of", "Original to", "Adopted from"];

fn extract_description(lines: &[String], rule_name: &str) -> String {
    let mut doc = String::new();
    for line in lines {
        let segment = line.trim();
        if segment.is_empty() {
            if !doc.is_empty() {
                break;
            }
            continue;
        }
        if !doc.is_empty() {
            doc.push(' ');
        }
        doc.push_str(segment);
        if doc.contains(". ") {
            break;
        }
    }
    if doc.is_empty() {
        eprintln!("warning: {rule_name}: no //! module doc found for description");
        return String::new();
    }

    // Take the first sentence (up to and including the first period followed by a
    // space, or the whole string if there is no sentence break).
    let first = match doc.find(". ") {
        Some(idx) => doc[..=idx].trim().to_string(),
        None => doc.trim().to_string(),
    };

    // Safety net: if the first sentence is itself a provenance note, drop it.
    if PROVENANCE.iter().any(|p| first.starts_with(p)) {
        return String::new();
    }
    first
}

/// Read the summary and full markdown docs, stripping provenance from the latter.
fn extract_docs(source_path: &Path, rule_name: &str) -> (String, String) {
    let content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "warning: {rule_name}: cannot read source {} for docs",
                source_path.display()
            );
            return (String::new(), String::new());
        }
    };

    // Collect the contiguous leading run of `//!` lines (blank `//!` lines
    // included as paragraph breaks). Stop at the first non-`//!` line once the
    // block has started.
    let mut lines: Vec<String> = Vec::new();
    let mut started = false;
    let mut description_before_code = true;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            started = true;
            // Drop exactly one leading space after the marker, if present.
            let text = rest.strip_prefix(' ').unwrap_or(rest);
            lines.push(text.to_string());
        } else if started {
            break;
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            description_before_code = false;
        }
        // Before the block starts, skip anything (blank lines, attributes).
    }
    let description = extract_description(
        if description_before_code { &lines } else { &[] },
        rule_name,
    );

    strip_provenance(&mut lines);

    // Trim leading and trailing blank lines so the markdown starts at the summary.
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        eprintln!("warning: {rule_name}: no //! module doc found for docs");
    }
    (description, lines.join("\n"))
}

/// Byte index of the period that ends a sentence: one followed by whitespace or
/// the end of the text. A period inside `List.filled` or `package:lints` does not
/// count, so a wrapped provenance sentence is cut at its real boundary.
fn sentence_end(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|&(i, c)| {
            c == '.'
                && text[i + 1..]
                    .chars()
                    .next()
                    .is_none_or(|next| next.is_whitespace())
        })
        .map(|(i, _)| i)
}

fn strip_provenance(lines: &mut Vec<String>) {
    let mut i = 0;
    while i < lines.len() {
        let Some(cut) = PROVENANCE.iter().filter_map(|p| lines[i].find(p)).min() else {
            i += 1;
            continue;
        };
        let end = sentence_end(&lines[i][cut..]);
        let wraps = end.is_none();
        if let Some(end) = end {
            let rest = cut + end + 1;
            let rest = rest + lines[i][rest..].len() - lines[i][rest..].trim_start().len();
            lines[i].replace_range(cut..rest, "");
        } else {
            lines[i].truncate(cut);
        }
        let trimmed_len = lines[i].trim_end().len();
        lines[i].truncate(trimmed_len);

        while wraps && i + 1 < lines.len() && !lines[i + 1].trim().is_empty() {
            match sentence_end(&lines[i + 1]) {
                // Anything after the closing period belongs to the next
                // sentence, so keep it.
                Some(p) => {
                    let rest = lines[i + 1][p + 1..].trim_start().to_string();
                    if rest.is_empty() {
                        lines.remove(i + 1);
                    } else {
                        lines[i + 1] = rest;
                    }
                    break;
                }
                None => {
                    lines.remove(i + 1);
                }
            }
        }
        if lines[i].is_empty() {
            lines.remove(i);
            if i > 0
                && lines[i - 1].trim().is_empty()
                && lines.get(i).is_some_and(|line| line.trim().is_empty())
            {
                lines.remove(i);
            }
        } else {
            i += 1;
        }
    }
}

fn expectation_segments(mut line: &str) -> impl Iterator<Item = (usize, &str)> {
    let length = line.len();
    std::iter::from_fn(move || {
        if line.is_empty() {
            return None;
        }
        let offset = length - line.len();
        if let Some(start) = line.find("/* expect:") {
            let before = &line[..start];
            line = line[start..]
                .find("*/")
                .map_or("", |end| &line[start + end + 2..]);
            Some((offset, before))
        } else {
            Some((offset, std::mem::take(&mut line)))
        }
    })
}

/// Remove all `/* expect: … */` annotation comments from fixture source, then
/// rstrip each line so the displayed code is clean.
fn strip_expectations(source: &str) -> String {
    let mut cleaned = String::with_capacity(source.len());
    for (i, line) in source.lines().enumerate() {
        if i > 0 {
            cleaned.push('\n');
        }
        let mut out = String::with_capacity(line.len());
        for (_, segment) in expectation_segments(line) {
            out.push_str(segment);
        }
        cleaned.push_str(out.trim_end());
    }
    cleaned
}

/// Human-facing source provenance for a rule.
fn source_json(source: &RuleSource) -> Value {
    let (kind, label, upstream_id, upstream_url): (&str, &str, Option<String>, Option<String>) =
        match source {
            RuleSource::Lints(id) => (
                "Lints",
                "Dart lints",
                Some((*id).to_string()),
                Some(format!("https://dart.dev/tools/linter-rules/{id}")),
            ),
            // DCM rule ids don't map to a stable per-rule URL without the rule's
            // DCM category, so keep the id + label but leave the URL null.
            RuleSource::DartCodeLinter(id) => (
                "DartCodeLinter",
                "Dart Code Metrics",
                Some((*id).to_string()),
                None,
            ),
            RuleSource::PyramidLint(id) => (
                "PyramidLint",
                "Pyramid Lint",
                Some((*id).to_string()),
                Some("https://pub.dev/packages/pyramid_lint".to_string()),
            ),
            RuleSource::Falcon => ("Falcon", "Falcon", None, None),
        };
    json!({
        "kind": kind,
        "label": label,
        "upstreamId": upstream_id,
        "upstreamUrl": upstream_url,
    })
}

/// Build the per-rule config passed to the falcon binary so that exactly this
/// rule fires (even non-recommended ones). Rules that ship a corpus
/// `config.json` (options/thresholds) reuse it verbatim; everything else gets a
/// minimal generated config written to `temp_path`.
fn docgen_config_for(corpus_root: &Path, meta: &RuleMeta, temp_path: &Path) -> Option<PathBuf> {
    let corpus_config = corpus_root.join(meta.name).join("config.json");
    if corpus_config.exists() {
        return Some(corpus_config);
    }

    let mut group_map = Map::new();
    group_map.insert(meta.name.to_string(), json!("error"));

    let mut rules = Map::new();
    rules.insert("recommended".to_string(), json!(false));
    rules.insert(meta.group.as_str().to_string(), Value::Object(group_map));

    let cfg = if meta.is_cross_file() {
        json!({
            "linter": { "rules": { "recommended": false } },
            "cross-file": { "rules": rules },
        })
    } else {
        json!({
            "linter": { "rules": rules },
            "cross-file": { "rules": { "recommended": false } },
        })
    };

    match fs::write(
        temp_path,
        serde_json::to_string_pretty(&cfg).unwrap_or_default(),
    ) {
        Ok(()) => Some(temp_path.to_path_buf()),
        Err(e) => {
            eprintln!(
                "warning: {}: cannot write temp config {}: {e}",
                meta.name,
                temp_path.display()
            );
            None
        }
    }
}

/// One CLI argument (option or positional) rendered from its clap definition.
fn cli_arg_json(arg: &clap::Arg) -> Value {
    // A flag (no value) carries no meaningful value name — only surface one for
    // args that actually take a value.
    let takes_value = arg
        .get_num_args()
        .map(|r| r.takes_values())
        .unwrap_or(false);
    let value_name = takes_value
        .then(|| {
            arg.get_value_names()
                .and_then(|vs| vs.first())
                .map(|v| v.to_string())
        })
        .flatten();

    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect();
    let default = match defaults.as_slice() {
        [] => Value::Null,
        [one] => json!(one),
        many => json!(many),
    };

    let possible_values: Vec<String> = arg
        .get_possible_values()
        .iter()
        .map(|pv| pv.get_name().to_string())
        .collect();

    json!({
        "name": arg.get_id().as_str(),
        "long": arg.get_long().map(|l| format!("--{l}")),
        "short": arg.get_short().map(|c| format!("-{c}")),
        "valueName": value_name,
        "default": default,
        "help": arg.get_help().map(|h| h.to_string()),
        "possibleValues": possible_values,
        "positional": arg.is_positional(),
    })
}

/// Whether an arg is a clap auto-generated help/version flag (excluded from the
/// reference — every command has them and they carry no falcon-specific meaning).
fn is_builtin_flag(arg: &clap::Arg) -> bool {
    matches!(arg.get_id().as_str(), "help" | "version")
}

/// Build the CLI reference from falcon's real clap `Command` tree: global args
/// plus one section per subcommand (name, about, usage, args).
fn cli_reference_json(mut root: clap::Command) -> Value {
    // Propagate globals into subcommands and finalize usage strings.
    root.build();

    let global_ids: HashSet<String> = root
        .get_arguments()
        .filter(|a| a.is_global_set())
        .map(|a| a.get_id().to_string())
        .collect();

    let mut globals: Vec<(&str, Value)> = root
        .get_arguments()
        .filter(|a| a.is_global_set() && !is_builtin_flag(a))
        .map(|a| (a.get_id().as_str(), cli_arg_json(a)))
        .collect();
    globals.sort_by(|a, b| a.0.cmp(b.0));
    let globals: Vec<Value> = globals.into_iter().map(|(_, v)| v).collect();

    // After `build()`, clap's rendered usage already carries the `falcon` bin
    // name; only the leading "Usage: " label needs trimming.
    let root_usage_raw = root.render_usage().to_string();
    let root_usage = root_usage_raw
        .strip_prefix("Usage: ")
        .unwrap_or(&root_usage_raw)
        .to_string();

    let mut subcommands: Vec<(String, Value)> = Vec::new();
    for sub in root.get_subcommands() {
        // Skip clap's auto-generated `help` subcommand — it is not a falcon command.
        if sub.get_name() == "help" {
            continue;
        }
        let mut sub = sub.clone();
        sub.build();
        let name = sub.get_name().to_string();

        let usage_raw = sub.render_usage().to_string();
        let usage = usage_raw
            .strip_prefix("Usage: ")
            .unwrap_or(&usage_raw)
            .to_string();

        let args: Vec<Value> = sub
            .get_arguments()
            .filter(|a| !global_ids.contains(a.get_id().as_str()) && !is_builtin_flag(a))
            .map(cli_arg_json)
            .collect();

        subcommands.push((
            name.clone(),
            json!({
                "name": name,
                "about": sub.get_about().map(|a| a.to_string()),
                "usage": usage,
                "args": args,
            }),
        ));
    }
    subcommands.sort_by(|a, b| a.0.cmp(&b.0));
    let subcommands: Vec<Value> = subcommands.into_iter().map(|(_, v)| v).collect();

    json!({
        "name": root.get_name(),
        "about": root.get_about().map(|a| a.to_string()),
        "version": root.get_version(),
        "usage": root_usage,
        "globalArgs": globals,
        "subcommands": subcommands,
    })
}

/// Resolve a `{"$ref": "#/definitions/X"}` node against the root schema, or
/// return the node unchanged when it is not a local `$ref`.
fn schema_resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    node.get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.strip_prefix("#/definitions/"))
        .and_then(|name| root.pointer(&format!("/definitions/{name}")))
        .unwrap_or(node)
}

/// A human-readable type label for a schema node (following `$ref`s).
fn schema_type_label(root: &Value, node: &Value) -> String {
    let resolved = schema_resolve(root, node);
    if resolved.get("oneOf").is_some() {
        // The only `oneOf` in the schema is `ruleConfig`.
        return "rule level (or object with level + options)".to_string();
    }
    match resolved.get("type") {
        Some(Value::String(s)) if s == "array" => {
            let item = resolved
                .get("items")
                .map(|i| schema_type_label(root, i))
                .unwrap_or_else(|| "value".to_string());
            format!("{item}[]")
        }
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" | "),
        _ if resolved.get("enum").is_some() => "string".to_string(),
        _ => "object".to_string(),
    }
}

/// The accepted values for a node, if it is an enum (or a `ruleConfig` whose
/// severity branch is `ruleLevel`).
fn schema_allowed_values(root: &Value, node: &Value) -> Vec<Value> {
    let resolved = schema_resolve(root, node);
    if let Some(Value::Array(e)) = resolved.get("enum") {
        return e.clone();
    }
    if let Some(Value::Array(branches)) = resolved.get("oneOf") {
        for branch in branches {
            let b = schema_resolve(root, branch);
            if let Some(Value::Array(e)) = b.get("enum") {
                return e.clone();
            }
        }
    }
    Vec::new()
}

/// Build one flat config-reference entry for a leaf key.
fn config_leaf(root: &Value, node: &Value, path: &str) -> Value {
    let resolved = schema_resolve(root, node);
    let description = node
        .get("description")
        .or_else(|| resolved.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut obj = Map::new();
    obj.insert("path".into(), json!(path));
    obj.insert("type".into(), json!(schema_type_label(root, node)));
    let allowed = schema_allowed_values(root, node);
    if !allowed.is_empty() {
        obj.insert("allowedValues".into(), Value::Array(allowed));
    }
    if let Some(default) = node.get("default").or_else(|| resolved.get("default")) {
        obj.insert("default".into(), default.clone());
    }
    obj.insert("description".into(), json!(description));
    Value::Object(obj)
}

/// Emit the two entries for a `rules` section: the `recommended` preset toggle
/// and a single templated `<path>.<group>.<rule>` entry (per-rule detail lives
/// in rules.json, so the reference stays structural).
fn emit_rules_section(root: &Value, node: &Value, path: &str, out: &mut Vec<Value>) {
    if let Some(rec) = node.pointer("/properties/recommended") {
        out.push(config_leaf(root, rec, &format!("{path}.recommended")));
    }
    let rule_config = root
        .pointer("/definitions/ruleConfig")
        .cloned()
        .unwrap_or(Value::Null);
    out.push(config_leaf(
        root,
        &rule_config,
        &format!("{path}.<group>.<rule>"),
    ));
}

/// Recursively walk the config schema, emitting a flat list of config-key
/// entries. Object keys recurse; `rules` sections and repetitive `overrides`
/// items collapse to templated paths; everything else is a leaf.
fn walk_config_schema(root: &Value, node: &Value, path: &str, out: &mut Vec<Value>) {
    let resolved = schema_resolve(root, node);

    if let Some(Value::Object(props)) = resolved.get("properties") {
        // A `recommended` property uniquely marks a `rules` section in this schema.
        if props.contains_key("recommended") {
            emit_rules_section(root, resolved, path, out);
            return;
        }
        for (key, child) in props {
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            walk_config_schema(root, child, &child_path, out);
        }
        return;
    }

    // Array of objects → descend into the item schema with a `[]` marker.
    if resolved.get("type").and_then(|t| t.as_str()) == Some("array")
        && let Some(items) = resolved.get("items")
        && matches!(
            schema_resolve(root, items).get("properties"),
            Some(Value::Object(_))
        )
    {
        walk_config_schema(root, items, &format!("{path}[]"), out);
        return;
    }

    out.push(config_leaf(root, node, path));
}

/// Build the per-key config reference by walking the committed
/// `schema/falcon.schema.json` (the metadata-generated source of truth).
fn config_reference_json(root: &Path) -> Value {
    let schema_path = root.join("schema/falcon.schema.json");
    let schema: Value = match fs::read_to_string(&schema_path) {
        Ok(s) => serde_json::from_str(&s).expect("schema is valid JSON"),
        Err(e) => {
            eprintln!(
                "error: cannot read {} for config reference: {e}",
                schema_path.display()
            );
            std::process::exit(1);
        }
    };
    let mut out: Vec<Value> = Vec::new();
    walk_config_schema(&schema, &schema, "", &mut out);
    Value::Array(out)
}

/// Emit `website/src/data/rules.json` and `website/src/data/domains.json` from
/// `RULE_METADATA`, corpus fixtures, and real diagnostics from the falcon
/// binary. These committed files are the docs website's only data source; the
/// site build never needs cargo.
fn docgen() {
    let root = workspace_root();
    let corpus_root = root.join("crates/falcon_rules/tests/corpus");

    let falcon_bin = build_falcon(false);
    let falcon = FalconRunner::new(falcon_bin, &root, &corpus_root);
    if RULE_METADATA.is_empty() {
        eprintln!("error: RULE_METADATA is empty");
        std::process::exit(1);
    }

    let temp_config = std::env::temp_dir().join("falcon-docgen-config.json");

    // Deterministic order: by group, then name.
    let mut metas: Vec<&RuleMeta> = RULE_METADATA.iter().collect();
    metas.sort_by(|a, b| {
        a.group
            .as_str()
            .cmp(b.group.as_str())
            .then(a.name.cmp(b.name))
    });

    let mut records: Vec<Value> = Vec::with_capacity(metas.len());
    let mut with_bad = 0usize;
    let mut with_good = 0usize;
    let mut with_diags = 0usize;
    let mut zero_diag_with_fixture: Vec<String> = Vec::new();

    for meta in &metas {
        let rule_dir = corpus_root.join(meta.name);
        let cross_file_dir = rule_dir.join("cross-file");

        let source_path = rule_source_path(&root, meta);
        let (description, docs) = extract_docs(&source_path, meta.name);

        let config = docgen_config_for(&corpus_root, meta, &temp_config);

        // Test-domain rules only inspect `_test.dart` files, so their examples come from those fixtures.
        let fixture = |stem: &str| {
            let test = rule_dir.join(format!("{stem}_test.dart"));
            if test.exists() {
                test
            } else {
                rule_dir.join(format!("{stem}.dart"))
            }
        };
        let bad_path = fixture("bad");
        let good_path = fixture("good");
        let bad_example = fs::read_to_string(&bad_path)
            .ok()
            .map(|s| strip_expectations(&s));
        let good_example = fs::read_to_string(&good_path)
            .ok()
            .map(|s| strip_expectations(&s));
        if bad_example.is_some() {
            with_bad += 1;
        }
        if good_example.is_some() {
            with_good += 1;
        }

        // Cross-file examples include nested Dart files and pubspecs. Relative
        // paths keep identically named files from independent packages distinct.
        let mut cross_file_sources: Vec<(String, String)> = Vec::new();
        if cross_file_dir.is_dir() {
            for file in recursive_files(&cross_file_dir).into_iter().filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("dart" | "yaml")
                )
            }) {
                if let Ok(source) = fs::read_to_string(&file)
                    && let Ok(relative) = file.strip_prefix(&cross_file_dir)
                {
                    cross_file_sources
                        .push((relative.to_string_lossy().replace('\\', "/"), source));
                }
            }
        }
        let cross_file_examples: Option<Value> = if cross_file_sources.is_empty() {
            None
        } else {
            Some(Value::Array(
                cross_file_sources
                    .iter()
                    .map(|(path, content)| {
                        json!({ "path": path, "content": strip_expectations(content) })
                    })
                    .collect(),
            ))
        };

        let mut diagnostics: Vec<Value> = Vec::new();
        let had_fixture;
        if meta.is_cross_file() && !cross_file_sources.is_empty() {
            had_fixture = true;
            for d in cross_file_project_roots(&cross_file_dir)
                .into_iter()
                .flat_map(|project| falcon.run_or_exit(&project, config.as_deref()))
                .filter(|d| d.rule == meta.name)
            {
                let relative = Path::new(&d.file_path)
                    .strip_prefix(&cross_file_dir)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| d.file_path.clone());
                let source = cross_file_sources
                    .iter()
                    .find(|(path, _)| path == &relative)
                    .map_or("", |(_, source)| source.as_str());
                diagnostics.push(json!({
                    "message": d.message,
                    "line": d.line,
                    "column": character_column(source, d.line, d.col),
                    "file": relative,
                }));
            }
        } else if bad_path.exists() {
            had_fixture = true;
            let source = fs::read_to_string(&bad_path).unwrap_or_default();
            for d in falcon
                .run_or_exit(&bad_path, config.as_deref())
                .into_iter()
                .filter(|d| d.rule == meta.name)
            {
                diagnostics.push(json!({
                    "message": d.message,
                    "line": d.line,
                    "column": character_column(&source, d.line, d.col),
                    "file": Value::Null,
                }));
            }
        } else {
            had_fixture = false;
        }

        if !diagnostics.is_empty() {
            with_diags += 1;
        } else if had_fixture {
            zero_diag_with_fixture.push(meta.name.to_string());
        }

        records.push(json!({
            "name": meta.name,
            "group": meta.group.as_str(),
            "domains": meta.domain_names(),
            "recommended": meta.recommended,
            "crossFile": meta.is_cross_file(),
            "source": source_json(&meta.source),
            "description": description,
            "docs": docs,
            "examples": {
                "bad": bad_example,
                "good": good_example,
                "crossFile": cross_file_examples,
            },
            "diagnostics": diagnostics,
        }));
    }

    let domains: Vec<Value> = Domain::ALL
        .iter()
        .map(|domain| {
            let mut rules: Vec<&str> = metas
                .iter()
                .filter(|m| m.domain == Some(*domain))
                .map(|m| m.name)
                .collect();
            rules.sort_unstable();
            json!({ "name": domain.as_str(), "rules": rules })
        })
        .collect();

    let data_dir = root.join("website/src/data");
    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("error: cannot create {}: {e}", data_dir.display());
        std::process::exit(1);
    }
    let rules_path = data_dir.join("rules.json");
    let domains_path = data_dir.join("domains.json");
    let cli_path = data_dir.join("cli.json");
    let config_ref_path = data_dir.join("config-reference.json");

    let rules_json = serde_json::to_string_pretty(&records).expect("serialize rules.json");
    let domains_json =
        serde_json::to_string_pretty(&Value::Array(domains)).expect("serialize domains.json");
    let cli_json = serde_json::to_string_pretty(&cli_reference_json(falcon_cli::args::command()))
        .expect("serialize cli.json");
    let config_ref = config_reference_json(&root);
    let config_ref_count = config_ref.as_array().map(|a| a.len()).unwrap_or(0);
    let config_ref_json =
        serde_json::to_string_pretty(&config_ref).expect("serialize config-reference.json");

    for (path, contents) in [
        (&rules_path, &rules_json),
        (&domains_path, &domains_json),
        (&cli_path, &cli_json),
        (&config_ref_path, &config_ref_json),
    ] {
        if let Err(e) = fs::write(path, format!("{contents}\n")) {
            eprintln!("error: cannot write {}: {e}", path.display());
            std::process::exit(1);
        }
    }

    println!();
    println!("── docgen ──────────────────────────────────────────────");
    println!("  Rules:              {}", records.len());
    println!("  With bad example:   {with_bad}");
    println!("  With good example:  {with_good}");
    println!("  With diagnostics:   {with_diags}");
    println!(
        "  Cross-file rules:   {}",
        metas.iter().filter(|m| m.is_cross_file()).count()
    );
    if !zero_diag_with_fixture.is_empty() {
        println!(
            "  Zero-diag fixtures: {} ({})",
            zero_diag_with_fixture.len(),
            zero_diag_with_fixture.join(", ")
        );
    }
    println!("  Config-ref keys:    {config_ref_count}");
    println!("  Wrote: {}", rules_path.display());
    println!("  Wrote: {}", domains_path.display());
    println!("  Wrote: {}", cli_path.display());
    println!("  Wrote: {}", config_ref_path.display());
    println!("────────────────────────────────────────────────────────");
}

#[cfg(test)]
mod tests {
    use super::{Cli, Task, strip_provenance, versioned_files, with_version_references};
    use clap::Parser;
    use std::path::Path;

    #[test]
    fn pinned_version_references_match_the_workspace_version() {
        let version = env!("CARGO_PKG_VERSION");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for path in versioned_files(root) {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text == with_version_references(&text, version),
                "{} pins a version other than {version}; run `cargo xtask version {version}`",
                path.display()
            );
        }
    }

    #[test]
    fn versioned_files_excludes_historical_changelog_pins() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("docs")).unwrap();
        let changelog = root.join("CHANGELOG.md");
        std::fs::write(&changelog, "Download falcon-0.1.0-x86_64-linux.tar.gz").unwrap();
        let readme = root.join("README.md");
        std::fs::write(&readme, "Download falcon-0.4.2-x86_64-linux.tar.gz").unwrap();

        let files = versioned_files(root);
        assert!(!files.contains(&changelog));
        assert!(files.contains(&readme));
    }

    #[test]
    fn launcher_version_references_rewrite_actual_files() {
        let version = env!("CARGO_PKG_VERSION");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let files = versioned_files(root);
        for (relative, prefix, suffix) in [
            ("packages/falcon/pubspec.yaml", "version: ", ""),
            (
                "packages/falcon/lib/src/version.dart",
                "const version = '",
                "';",
            ),
        ] {
            let path = root.join(relative);
            assert!(files.contains(&path), "{relative} must be versioned");
            let text = std::fs::read_to_string(path).unwrap();
            let current = format!("{prefix}{version}{suffix}");
            assert!(text.lines().any(|line| line == current));
            let bumped = with_version_references(&text, "99.0.0");
            assert_eq!(
                bumped,
                text.replace(&current, &format!("{prefix}99.0.0{suffix}"))
            );
            assert_ne!(bumped, text);
        }
    }

    #[test]
    fn version_references_rewrite_pins_and_nothing_else() {
        let text = r#"releases/download/v0.4.1/falcon-0.4.1-x86_64-linux.tar.gz
"github:JacobDevelops/dart_falcon" = { version = "0.4.1", matching = "falcon-" }
"version": "0.4.1"
version: 0.4.1
const version = '0.4.1';
  sdk: ^3.13.0
releases/latest/download/falcon-x86_64-linux.tar.gz regex 0.4.17"#;
        assert_eq!(
            with_version_references(text, "0.5.0"),
            r#"releases/download/v0.5.0/falcon-0.5.0-x86_64-linux.tar.gz
"github:JacobDevelops/dart_falcon" = { version = "0.5.0", matching = "falcon-" }
"version": "0.5.0"
version: 0.5.0
const version = '0.5.0';
  sdk: ^3.13.0
releases/latest/download/falcon-x86_64-linux.tar.gz regex 0.4.17"#
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

    #[cfg(unix)]
    #[test]
    fn runner_distinguishes_diagnostic_exit_codes_from_process_failures() {
        use std::os::unix::fs::PermissionsExt;
        for (status, stdout, expected_error) in [
            (2, "[]", Some("exit status: 2")),
            (1, "", Some("JSON parse error")),
            (1, "[]", None),
            (0, "[]", None),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let binary = directory.path().join("falcon");
            std::fs::write(
                &binary,
                format!(
                    "#!/bin/sh\nprintf '{stdout}'\nprintf 'fixture failure' >&2\nexit {status}\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
            let runner = super::FalconRunner {
                binary: binary.to_string_lossy().into_owned(),
                fallback_sdk: None,
            };
            let result = runner.run(&directory.path().join("clean.dart"), None);
            if let Some(expected) = expected_error {
                let error = result.unwrap_err();
                assert!(error.contains("fixture failure"), "{error}");
                assert!(error.contains(expected), "{error}");
            } else {
                assert!(result.unwrap().is_empty());
            }
        }
    }

    #[test]
    fn fixture_sdk_is_always_used_for_the_bundled_corpus() {
        let workspace = super::workspace_root();
        let bundled = workspace.join("crates/falcon_rules/tests/corpus");
        let fixture = workspace.join("crates/falcon_rules/tests/support/sdk");
        // Constructing the runner does not need or consult an ambient SDK locator.
        let runner = super::FalconRunner::new(
            std::path::PathBuf::from("unused-falcon"),
            &workspace,
            &bundled,
        );
        assert_eq!(runner.fallback_sdk, Some(fixture));
        let custom = workspace.join("target/custom-corpus");
        assert_eq!(super::corpus_fixture_sdk(&workspace, &custom), None);
    }

    #[test]
    fn fixture_sdk_only_applies_to_listed_source_files() {
        let workspace = super::workspace_root();
        let sdk = workspace.join("crates/falcon_rules/tests/support/sdk");
        let runner = super::FalconRunner {
            binary: String::from("falcon"),
            fallback_sdk: Some(sdk.clone()),
        };
        let bundled = workspace.join("crates/falcon_rules/tests/corpus");
        assert_eq!(
            runner.sdk_for(&bundled.join("omit-local-variable-types/enum_values.dart")),
            Some(sdk.as_path()),
        );
        assert_eq!(
            runner.sdk_for(&bundled.join("omit-local-variable-types/good.dart")),
            None,
        );
        assert_eq!(runner.sdk_for(&bundled), None);
        let custom_runner = super::FalconRunner {
            binary: String::from("falcon"),
            fallback_sdk: super::corpus_fixture_sdk(
                &workspace,
                &workspace.join("target/custom-corpus"),
            ),
        };
        assert_eq!(
            custom_runner.sdk_for(
                &workspace.join("target/custom-corpus/omit-local-variable-types/enum_values.dart"),
            ),
            None,
        );
    }

    #[test]
    fn fixture_inventory_ignores_blank_lines_and_surrounding_whitespace() {
        let fixtures = "\n \t\n omit-local-variable-types/enum_values.dart \n\n";
        assert!(super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("corpus/omit-local-variable-types/enum_values.dart"),
            fixtures,
        ));
        assert!(!super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("corpus/omit-local-variable-types/good.dart"),
            fixtures,
        ));
        assert!(!super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("anything.dart"),
            "\n \t\n",
        ));
    }

    #[test]
    fn docgen_columns_count_characters_not_utf16_units() {
        assert_eq!(super::character_column("x😀é target\n", 1, 6), 5);
        assert_eq!(super::character_column("first\n😀 target\n", 2, 4), 3);
        assert_eq!(super::character_column("plain target\n", 1, 7), 7);
        assert_eq!(
            super::character_column("/* expect: rule */ // ignore: unused_import", 1, 20),
            2
        );
        assert_eq!(
            super::character_column("é😀 /* expect: 😀 */ x /* expect: rule */ target", 1, 43),
            8
        );
        assert_eq!(
            super::character_column("x /* expect: rule */ target", 1, 7),
            3
        );
        assert_eq!(super::character_column("x /* expect: rule", 1, 18), 2);
        assert_eq!(
            super::character_column("target /* expect: rule */  ", 1, 27),
            7
        );
    }

    #[test]
    fn perf_lock_requires_root_unless_corpus_is_explicit() {
        use std::path::PathBuf;
        assert_eq!(super::perf_corpus(None, None), None);
        assert_eq!(
            super::perf_corpus(None, Some("/tmp/jfit".into())),
            Some(PathBuf::from("/tmp/jfit/apps/mobile/lib")),
        );
        assert_eq!(
            super::perf_corpus(Some(PathBuf::from("/tmp/corpus")), None),
            Some(PathBuf::from("/tmp/corpus")),
        );
    }

    fn run(input: &[&str]) -> Vec<String> {
        let mut lines: Vec<String> = input.iter().map(|s| s.to_string()).collect();
        strip_provenance(&mut lines);
        lines
    }

    #[test]
    fn validate_rules_binary_override_is_optional() {
        for (args, expected) in [
            (vec!["xtask", "validate-rules"], None),
            (
                vec![
                    "xtask",
                    "validate-rules",
                    "--falcon-bin",
                    "/tmp/custom-falcon",
                ],
                Some("/tmp/custom-falcon"),
            ),
        ] {
            let Task::ValidateRules(options) = Cli::try_parse_from(args).unwrap().task else {
                panic!("expected validate-rules");
            };
            assert_eq!(
                options.falcon_bin.as_deref(),
                expected.map(std::path::Path::new)
            );
        }
    }

    #[test]
    fn cuts_a_provenance_sentence_that_wraps_onto_the_next_line() {
        assert_eq!(
            run(&[
                "those have no literal form. Adopted from package:lints",
                "`prefer_collection_literals`.",
                "",
                "Two further facts."
            ]),
            vec!["those have no literal form.", "", "Two further facts."]
        );
    }

    #[test]
    fn keeps_content_following_the_closing_period() {
        assert_eq!(
            run(&[
                "intro. Adopted from package:lints",
                "`prefer_is_empty`. Only a plain access matches."
            ]),
            vec!["intro.", "Only a plain access matches."]
        );
    }

    #[test]
    fn keeps_description_after_provenance_on_the_same_line() {
        assert_eq!(
            run(&[
                "Summary. Adopted from the official `example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec!["Summary. Only plain accesses", "match; calls do not."]
        );
        assert_eq!(
            run(&[
                "Adopted from the official `example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec!["Only plain accesses", "match; calls do not."]
        );
    }

    #[test]
    fn removing_standalone_provenance_does_not_add_a_paragraph_break() {
        assert_eq!(
            run(&[
                "Summary.",
                "",
                "Adopted from the official",
                "`example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec![
                "Summary.",
                "",
                "Only plain accesses",
                "match; calls do not."
            ]
        );
        assert_eq!(
            run(&[
                "Summary.",
                "",
                "Adopted from the official `example` lint.",
                "",
                "Description.",
            ]),
            vec!["Summary.", "", "Description."]
        );
    }

    #[test]
    fn leaves_a_single_line_sentence_and_unrelated_text_alone() {
        assert_eq!(
            run(&[
                "Flags a thing. Adopted from package:lints `prefer_iterable_whereType`.",
                "Body stays."
            ]),
            vec!["Flags a thing.", "Body stays."]
        );
    }

    #[test]
    fn a_period_inside_an_identifier_is_not_a_sentence_boundary() {
        assert_eq!(
            run(&[
                "intro. Adopted from package:lints",
                "`List.filled` and `prefer_collection_literals`. Body stays."
            ]),
            vec!["intro.", "Body stays."]
        );
    }

    #[test]
    fn stops_at_a_paragraph_break_rather_than_eating_the_next_block() {
        assert_eq!(
            run(&["intro. Ported from dart_code_linter", "", "Next paragraph."]),
            vec!["intro.", "", "Next paragraph."]
        );
    }
}
