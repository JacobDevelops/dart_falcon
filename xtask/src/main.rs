use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("codegen") => codegen(&args[1..]),
        Some("validate-rules") => validate_rules(&args[1..]),
        Some("perf-lock") => perf_lock(&args[1..]),
        Some("schema") => schema(&args[1..]),
        Some("docgen") => docgen(&args[1..]),
        _ => {
            eprintln!("Usage: cargo xtask <task>");
            eprintln!("Available tasks:");
            eprintln!("  codegen         Generate rule implementation + fixture stubs");
            eprintln!("  validate-rules  Validate rule implementations against golden corpus");
            eprintln!(
                "  docgen          Emit website/src/data/{{rules,domains}}.json for the docs site"
            );
            eprintln!(
                "  perf-lock       Enforce the M6 performance lock (<1000ms on jfit mobile lib)"
            );
            eprintln!(
                "  schema          Generate schema/falcon.schema.json (--check to verify drift)"
            );
            eprintln!();
            eprintln!("codegen usage:");
            eprintln!(
                "  cargo xtask codegen rule --group <complexity|correctness|performance|style|suspicious> --name <rule-id>"
            );
            eprintln!();
            eprintln!("validate-rules flags:");
            eprintln!(
                "  --corpus <path>      Path to corpus directory (default: crates/falcon_rules/tests/corpus)"
            );
            eprintln!(
                "  --threshold <float>  Fuzzy message match threshold 0.0-1.0 (default: 0.85)"
            );
            eprintln!("  --rule <name>        Filter to a single rule");
            eprintln!(
                "  --falcon-bin <path>  Path to falcon binary (default: target/debug/falcon)"
            );
            eprintln!("  --json               Output results as JSON");
            eprintln!();
            eprintln!("perf-lock flags:");
            eprintln!(
                "  --corpus <path>      Corpus to lint (default: $JFIT_PATH or /home/jacob/Documents/Developer/jfit, subtree apps/mobile/lib)"
            );
            eprintln!("  --runs <N>           Timed runs to take the median of (default: 5)");
            eprintln!("  --budget-ms <N>      Wall-clock budget in milliseconds (default: 1000)");
            eprintln!(
                "  --falcon-bin <path>  Path to falcon binary (default: target/release/falcon)"
            );
            eprintln!("  --skip-build         Don't rebuild the release binary first");
            eprintln!("  --json               Output results as JSON");
            std::process::exit(1);
        }
    }
}

// ── Schema ──────────────────────────────────────────────────────────────────

/// Generate `schema/falcon.schema.json` from the rule metadata, or with
/// `--check` verify the committed file matches (CI drift guard).
fn schema(args: &[String]) {
    let check = args.iter().any(|a| a == "--check");
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

// ── Codegen ─────────────────────────────────────────────────────────────────

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
fn rule_stub(struct_name: &str, rule_id: &str) -> String {
    format!(
        r#"use falcon_analyze::{{AnalyzeContext, Rule}};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct {struct_name};

impl Rule for {struct_name} {{
    fn name(&self) -> &'static str {{
        "{rule_id}"
    }}

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {{
        let _ = (program, ctx);
        // TODO: walk `program.declarations` and emit diagnostics per violation.
        Vec::new()
    }}
}}
"#
    )
}

const GROUPS: [&str; 5] = [
    "complexity",
    "correctness",
    "performance",
    "style",
    "suspicious",
];

/// Generate a rule implementation stub plus corpus fixture skeletons.
///
/// `cargo xtask codegen rule --group suspicious --name avoid-foo`
///
/// The rule id is taken from `--name` verbatim (no kebab/snake derivation); the
/// module file is `lint/<group>/<name-with-dashes-as-underscores>.rs`.
fn codegen(args: &[String]) {
    if args.first().map(|s| s.as_str()) != Some("rule") {
        eprintln!(
            "Usage: cargo xtask codegen rule --group <{}> --name <rule-id>",
            GROUPS.join("|")
        );
        std::process::exit(1);
    }

    let mut name: Option<String> = None;
    let mut group: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                i += 1;
                name = args.get(i).cloned();
            }
            "--group" => {
                i += 1;
                group = args.get(i).cloned();
            }
            other => {
                eprintln!("error: unknown codegen flag: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let Some(rule_id) = name else {
        eprintln!("error: --name is required (the rule id, used verbatim)");
        std::process::exit(1);
    };
    let Some(group) = group else {
        eprintln!("error: --group is required");
        std::process::exit(1);
    };
    if !GROUPS.contains(&group.as_str()) {
        eprintln!("error: --group must be one of {}", GROUPS.join(", "));
        std::process::exit(1);
    }

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
    fs::write(&rule_file, rule_stub(&struct_name, &rule_id)).expect("write rule stub");

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
    println!(
        "  1. add `pub mod {};` to crates/falcon_rules/src/lint/{}.rs",
        module, group
    );
    println!(
        "  2. register `Box::new(lint::{}::{}::{})` in all_rules() (crates/falcon_rules/src/lib.rs)",
        group, module, struct_name
    );
    println!(
        "  3. add a `RuleMeta {{ name: \"{}\", group: \"{}\", domains, recommended, cross_file: false, \
         source: RuleSource::Falcon }}` entry to RULE_METADATA (crates/falcon_rules/src/meta.rs)",
        rule_id, group
    );
    println!(
        "  4. implement the rule, fill in fixtures, then run `cargo xtask validate-rules --rule {}`",
        rule_id
    );
}

// ── Validation harness ──────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    // When run via `cargo xtask`, CWD is the workspace root.
    std::env::current_dir().unwrap()
}

/// Convert a byte offset in `source` to a 1-indexed line number.
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

#[derive(Debug, Clone)]
struct Expectation {
    rule: String,
    line: usize,
    expected_msg: Option<String>,
}

/// Parse `/* expect: rule-name */` or `/* expect: rule-name, msg: "text" */` annotations.
/// The annotation appears on the same line as the expected violation.
fn parse_expectations(source: &str) -> Vec<Expectation> {
    let mut exps = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx + 1;
        let mut search = line;
        while let Some(start) = search.find("/* expect:") {
            let after = &search[start + 10..];
            if let Some(end) = after.find("*/") {
                let annotation = after[..end].trim();
                // annotation: "rule-name" or "rule-name, msg: \"text\""
                let mut parts = annotation.splitn(2, ',');
                let rule = parts.next().unwrap_or("").trim().to_string();
                let msg_part = parts.next().map(|s| s.trim());
                let expected_msg = msg_part.and_then(|s| {
                    if let Some(rest) = s.strip_prefix("msg:") {
                        let trimmed = rest.trim().trim_matches('"');
                        Some(trimmed.to_string())
                    } else {
                        None
                    }
                });
                if !rule.is_empty() {
                    exps.push(Expectation {
                        rule,
                        line: line_num,
                        expected_msg,
                    });
                }
                search = &after[end + 2..];
            } else {
                break;
            }
        }
    }
    exps
}

#[derive(Debug, serde::Deserialize)]
struct DiagnosticJson {
    rule: String,
    message: String,
    span: SpanJson,
    #[allow(dead_code)]
    severity: String,
    #[allow(dead_code)]
    #[serde(default)]
    file_path: String,
}

#[derive(Debug, serde::Deserialize)]
struct SpanJson {
    start: usize,
    #[allow(dead_code)]
    end: usize,
}

fn run_falcon(
    falcon_bin: &str,
    file: &Path,
    config: Option<&Path>,
) -> Result<Vec<DiagnosticJson>, String> {
    let mut cmd = Command::new(falcon_bin);
    cmd.args(["check", "--format", "json", file.to_str().unwrap_or("")]);
    if let Some(config_path) = config {
        cmd.args(["--config", config_path.to_str().unwrap_or("")]);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run falcon binary '{}': {}", falcon_bin, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<DiagnosticJson>>(trimmed).map_err(|e| {
        format!(
            "JSON parse error: {}\nOutput was: {}",
            e,
            &trimmed[..trimmed.len().min(200)]
        )
    })
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
    #[allow(dead_code)]
    file: PathBuf,
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
    falcon_bin: &str,
    threshold: f64,
    rule_filter: Option<&str>,
    config: Option<&Path>,
) -> FileResult {
    let all_exps = parse_expectations(source);
    let exps: Vec<Expectation> = all_exps
        .into_iter()
        .filter(|e| rule_filter.map(|r| e.rule == r).unwrap_or(true))
        .collect();

    let raw_diags = run_falcon(falcon_bin, file, config).unwrap_or_default();
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
            let diag_line = byte_offset_to_line(source, diag.span.start);
            if diag.rule != exp.rule || diag_line != exp.line {
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
        .map(|(_, d)| {
            let line = byte_offset_to_line(source, d.span.start);
            UnexpectedDiagnostic {
                rule: d.rule.clone(),
                line,
                message: d.message.clone(),
            }
        })
        .collect();

    FileResult {
        file: file.to_path_buf(),
        expected: exps.len(),
        matched,
        missed,
        false_positives,
    }
}

/// Validate a cross-file rule's multi-file fixture directory
/// (`corpus/<rule>/cross-file/`) as ONE falcon invocation over the whole
/// directory. Expectations are matched by (file name, line), since cross-file
/// diagnostics can land in any of the fixture's files. Returns (expected,
/// matched, failures).
fn validate_cross_file_dir(
    cross_file_dir: &Path,
    rule_name: &str,
    falcon_bin: &str,
    config: Option<&Path>,
) -> (usize, usize, Vec<String>) {
    // Map each fixture file's basename → its source (for offset→line lookup).
    let mut sources: HashMap<String, String> = HashMap::new();
    let mut dart_files: Vec<PathBuf> = fs::read_dir(cross_file_dir)
        .unwrap_or_else(|_| panic!("cannot read {}", cross_file_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("dart"))
        .collect();
    dart_files.sort();

    let mut expectations: Vec<(String, usize)> = Vec::new();
    for file in &dart_files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let base = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for exp in parse_expectations(&source) {
            if exp.rule == rule_name {
                expectations.push((base.clone(), exp.line));
            }
        }
        sources.insert(base, source);
    }

    let diags = run_falcon(falcon_bin, cross_file_dir, config).unwrap_or_default();
    let mut diag_keys: Vec<(String, usize)> = diags
        .iter()
        .filter(|d| d.rule == rule_name)
        .map(|d| {
            let base = Path::new(&d.file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let line = sources
                .get(&base)
                .map(|s| byte_offset_to_line(s, d.span.start))
                .unwrap_or(0);
            (base, line)
        })
        .collect();

    let expected = expectations.len();
    let mut matched = 0usize;
    let mut failures = Vec::new();
    for exp in &expectations {
        if let Some(pos) = diag_keys.iter().position(|k| k == exp) {
            diag_keys.remove(pos);
            matched += 1;
        } else {
            failures.push(format!(
                "MISS  {}/{}:{} — rule `{}` expected but not emitted",
                cross_file_dir.display(),
                exp.0,
                exp.1,
                rule_name
            ));
        }
    }
    for key in diag_keys {
        failures.push(format!(
            "EXTRA {}/{}:{} — rule `{}` fired unexpectedly",
            cross_file_dir.display(),
            key.0,
            key.1,
            rule_name
        ));
    }
    (expected, matched, failures)
}

fn validate_rules(args: &[String]) {
    let workspace = workspace_root();
    let default_corpus = workspace.join("crates/falcon_rules/tests/corpus");
    let default_bin = workspace.join("target/debug/falcon");

    let mut corpus_path = default_corpus;
    let mut threshold: f64 = 0.85;
    let mut rule_filter: Option<String> = None;
    let mut falcon_bin = default_bin.to_string_lossy().to_string();
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--corpus" => {
                i += 1;
                if i < args.len() {
                    corpus_path = PathBuf::from(&args[i]);
                }
            }
            "--threshold" => {
                i += 1;
                if i < args.len() {
                    threshold = args[i].parse().unwrap_or(0.85);
                }
            }
            "--rule" => {
                i += 1;
                if i < args.len() {
                    rule_filter = Some(args[i].clone());
                }
            }
            "--falcon-bin" => {
                i += 1;
                if i < args.len() {
                    falcon_bin = args[i].clone();
                }
            }
            "--json" => {
                json_output = true;
            }
            _ => {}
        }
        i += 1;
    }

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

    let mut total_files: usize = 0;
    let mut total_expected: usize = 0;
    let mut total_matched: usize = 0;
    let mut all_failures: Vec<String> = Vec::new();

    for rule_dir in &rule_dirs {
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

        // Config-gated rules ship a per-rule config (`corpus/<rule>/config.json`,
        // full falcon.json shape) that is passed to the falcon binary via --config.
        let rule_config_path = rule_dir.join("config.json");
        let rule_config = rule_config_path.exists().then_some(rule_config_path);

        for dart_file in &dart_files {
            let source = match fs::read_to_string(dart_file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("warning: cannot read {}: {}", dart_file.display(), e);
                    continue;
                }
            };

            let result = validate_file(
                dart_file,
                &source,
                &falcon_bin,
                threshold,
                Some(&rule_name),
                rule_config.as_deref(),
            );
            total_files += 1;
            total_expected += result.expected;
            total_matched += result.matched;

            for m in &result.missed {
                let msg = format!(
                    "MISS  {}:{} — rule `{}` expected but not emitted",
                    dart_file.display(),
                    m.line,
                    m.rule
                );
                all_failures.push(msg.clone());
                if !json_output {
                    eprintln!("{msg}");
                }
            }
            for fp in &result.false_positives {
                let msg = format!(
                    "EXTRA {}:{} — rule `{}` fired unexpectedly: {}",
                    dart_file.display(),
                    fp.line,
                    fp.rule,
                    fp.message
                );
                all_failures.push(msg.clone());
                if !json_output {
                    eprintln!("{msg}");
                }
            }
        }

        // Cross-file rules keep their fixtures in a `cross-file/` subdirectory,
        // run as one invocation over the whole directory.
        let cross_file_dir = rule_dir.join("cross-file");
        if cross_file_dir.is_dir() {
            let (expected, matched, failures) = validate_cross_file_dir(
                &cross_file_dir,
                &rule_name,
                &falcon_bin,
                rule_config.as_deref(),
            );
            total_files += 1;
            total_expected += expected;
            total_matched += matched;
            for msg in failures {
                all_failures.push(msg.clone());
                if !json_output {
                    eprintln!("{msg}");
                }
            }
        }
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

// ── Perf lock ───────────────────────────────────────────────────────────────

/// M6.3 performance lock: the release falcon binary must lint the jfit mobile
/// lib in under the budget (default 1000ms, plan M6 exit criterion). Runs the
/// binary N times and compares the median wall time against the budget.
fn perf_lock(args: &[String]) {
    let workspace = workspace_root();

    let mut corpus: Option<PathBuf> = None;
    let mut runs: usize = 5;
    let mut budget_ms: u128 = 1000;
    let mut falcon_bin = workspace.join("target/release/falcon");
    let mut skip_build = false;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--corpus" => {
                i += 1;
                if i < args.len() {
                    corpus = Some(PathBuf::from(&args[i]));
                }
            }
            "--runs" => {
                i += 1;
                if i < args.len() {
                    runs = args[i].parse().unwrap_or(5).max(1);
                }
            }
            "--budget-ms" => {
                i += 1;
                if i < args.len() {
                    budget_ms = args[i].parse().unwrap_or(1000);
                }
            }
            "--falcon-bin" => {
                i += 1;
                if i < args.len() {
                    falcon_bin = PathBuf::from(&args[i]);
                }
            }
            "--skip-build" => skip_build = true,
            "--json" => json_output = true,
            _ => {}
        }
        i += 1;
    }

    let corpus = corpus.unwrap_or_else(|| {
        let root = std::env::var("JFIT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/jacob/Documents/Developer/jfit"));
        root.join("apps/mobile/lib")
    });
    if !corpus.exists() {
        eprintln!("error: corpus path does not exist: {}", corpus.display());
        std::process::exit(1);
    }

    if !skip_build {
        eprintln!("building release falcon binary...");
        let status = Command::new("cargo")
            .args(["build", "--release", "--bin", "falcon"])
            .current_dir(&workspace)
            .status()
            .expect("failed to spawn cargo build");
        if !status.success() {
            eprintln!("error: release build failed");
            std::process::exit(1);
        }
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
            .args(["check", "--quiet", "--parallel", "--exit-code", "0"])
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

// ── Docgen ──────────────────────────────────────────────────────────────────

use falcon_rules::meta::{DOMAINS, RULE_METADATA, RuleMeta, RuleSource};
use serde_json::{Map, Value, json};

/// Convert a byte offset in `source` to a 1-indexed column (character count from
/// the start of the line).
fn byte_offset_to_col(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    let line_start = source[..clamped].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..clamped].chars().count() + 1
}

/// Map a rule's snake_case module name from its kebab id.
fn snake_of(name: &str) -> String {
    name.replace('-', "_")
}

/// The source file path for a rule's implementation.
fn rule_source_path(root: &Path, meta: &RuleMeta) -> PathBuf {
    let snake = snake_of(meta.name);
    if meta.cross_file {
        root.join(format!("crates/falcon_rules/src/cross_file/{snake}.rs"))
    } else {
        root.join(format!(
            "crates/falcon_rules/src/lint/{}/{}.rs",
            meta.group, snake
        ))
    }
}

/// Extract the lead description from a rule's `//!` module doc: the first
/// sentence, with any trailing provenance sentence ("Ported from …", "Port of
/// …", "Original to falcon.", "Adopted from …") dropped. Returns "" (and warns)
/// when the file or doc is missing.
fn extract_description(source_path: &Path, rule_name: &str) -> String {
    let content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "warning: {rule_name}: cannot read source {} for description",
                source_path.display()
            );
            return String::new();
        }
    };

    // Join the leading run of `//!` doc lines until the first complete sentence.
    let mut doc = String::new();
    let mut saw_doc = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            saw_doc = true;
            let seg = rest.trim();
            if seg.is_empty() {
                if !doc.is_empty() {
                    break;
                }
                continue;
            }
            if !doc.is_empty() {
                doc.push(' ');
            }
            doc.push_str(seg);
            if doc.contains(". ") {
                break;
            }
        } else if saw_doc {
            break;
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Hit real code before any module doc — none present.
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
    const PROVENANCE: [&str; 4] = ["Ported from", "Port of", "Original to", "Adopted from"];
    if PROVENANCE.iter().any(|p| first.starts_with(p)) {
        return String::new();
    }
    first
}

/// Extract the FULL `//!` module doc from a rule's source as markdown: every
/// leading `//!` line with the `//! ` marker stripped, joined with newlines. A
/// trailing provenance sentence ("Ported from …", "Port of …", "Original to …",
/// "Adopted from …") is cut from any line that still carries one, and the block
/// is trimmed of leading/blank framing. Returns "" when no module doc is present.
fn extract_full_docs(source_path: &Path, rule_name: &str) -> String {
    const PROVENANCE: [&str; 4] = ["Ported from", "Port of", "Original to", "Adopted from"];

    let content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "warning: {rule_name}: cannot read source {} for docs",
                source_path.display()
            );
            return String::new();
        }
    };

    // Collect the contiguous leading run of `//!` lines (blank `//!` lines
    // included as paragraph breaks). Stop at the first non-`//!` line once the
    // block has started.
    let mut lines: Vec<String> = Vec::new();
    let mut started = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            started = true;
            // Drop exactly one leading space after the marker, if present.
            let text = rest.strip_prefix(' ').unwrap_or(rest);
            lines.push(text.to_string());
        } else if started {
            break;
        }
        // Before the block starts, skip anything (blank lines, attributes).
    }

    // Cut a trailing provenance sentence from any line that still carries one.
    for line in &mut lines {
        if let Some(cut) = PROVENANCE
            .iter()
            .filter_map(|p| line.find(p))
            .min()
        {
            line.truncate(cut);
            let trimmed_len = line.trim_end().len();
            line.truncate(trimmed_len);
        }
    }

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
    lines.join("\n")
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
        let mut rest = line;
        while let Some(start) = rest.find("/* expect:") {
            out.push_str(&rest[..start]);
            match rest[start..].find("*/") {
                Some(end) => rest = &rest[start + end + 2..],
                None => {
                    rest = "";
                    break;
                }
            }
        }
        out.push_str(rest);
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
fn docgen_config_for(root: &Path, meta: &RuleMeta, temp_path: &Path) -> Option<PathBuf> {
    let corpus_config = root.join(format!(
        "crates/falcon_rules/tests/corpus/{}/config.json",
        meta.name
    ));
    if corpus_config.exists() {
        return Some(corpus_config);
    }

    let mut group_map = Map::new();
    group_map.insert(meta.name.to_string(), json!("error"));

    let mut rules = Map::new();
    rules.insert("recommended".to_string(), json!(false));
    rules.insert(meta.group.to_string(), Value::Object(group_map));

    let cfg = if meta.cross_file {
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

// ── CLI reference (cli.json) ─────────────────────────────────────────────────

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

// ── Config reference (config-reference.json) ─────────────────────────────────

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
        && matches!(schema_resolve(root, items).get("properties"), Some(Value::Object(_)))
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
fn docgen(_args: &[String]) {
    let root = workspace_root();
    let corpus_root = root.join("crates/falcon_rules/tests/corpus");

    // Build the debug falcon binary once, up front.
    eprintln!("building falcon binary (debug)...");
    let status = Command::new("cargo")
        .args(["build", "--bin", "falcon"])
        .current_dir(&root)
        .status()
        .expect("failed to spawn cargo build");
    if !status.success() {
        eprintln!("error: falcon build failed");
        std::process::exit(1);
    }
    let falcon_bin = root.join("target/debug/falcon");
    let falcon_bin = falcon_bin.to_string_lossy().to_string();
    if RULE_METADATA.is_empty() {
        eprintln!("error: RULE_METADATA is empty");
        std::process::exit(1);
    }

    let temp_config = std::env::temp_dir().join("falcon-docgen-config.json");

    // Deterministic order: by group, then name.
    let mut metas: Vec<&RuleMeta> = RULE_METADATA.iter().collect();
    metas.sort_by(|a, b| a.group.cmp(b.group).then(a.name.cmp(b.name)));

    let mut records: Vec<Value> = Vec::with_capacity(metas.len());
    let mut with_bad = 0usize;
    let mut with_good = 0usize;
    let mut with_diags = 0usize;
    let mut zero_diag_with_fixture: Vec<String> = Vec::new();

    for meta in &metas {
        let rule_dir = corpus_root.join(meta.name);
        let cross_file_dir = rule_dir.join("cross-file");

        let source_path = rule_source_path(&root, meta);
        let description = extract_description(&source_path, meta.name);
        let docs = extract_full_docs(&source_path, meta.name);

        let config = docgen_config_for(&root, meta, &temp_config);

        // ── examples ──
        let bad_path = rule_dir.join("bad.dart");
        let good_path = rule_dir.join("good.dart");
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

        // Cross-file fixture files (bad side): every .dart under cross-file/.
        let mut cross_file_sources: Vec<(String, String)> = Vec::new();
        if cross_file_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&cross_file_dir)
        {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("dart"))
                .collect();
            files.sort();
            for f in files {
                if let Ok(src) = fs::read_to_string(&f) {
                    let base = f
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    cross_file_sources.push((base, src));
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

        // ── diagnostics ── real output from the falcon binary, filtered to this rule.
        let mut diagnostics: Vec<Value> = Vec::new();
        let had_fixture;
        if meta.cross_file && !cross_file_sources.is_empty() {
            had_fixture = true;
            let by_base: HashMap<&str, &str> = cross_file_sources
                .iter()
                .map(|(b, s)| (b.as_str(), s.as_str()))
                .collect();
            for d in run_falcon(&falcon_bin, &cross_file_dir, config.as_deref())
                .unwrap_or_default()
                .into_iter()
                .filter(|d| d.rule == meta.name)
            {
                let base = Path::new(&d.file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let src = by_base.get(base.as_str()).copied().unwrap_or("");
                diagnostics.push(json!({
                    "message": d.message,
                    "line": byte_offset_to_line(src, d.span.start),
                    "column": byte_offset_to_col(src, d.span.start),
                    "file": base,
                }));
            }
        } else if bad_path.exists() {
            had_fixture = true;
            let src = fs::read_to_string(&bad_path).unwrap_or_default();
            for d in run_falcon(&falcon_bin, &bad_path, config.as_deref())
                .unwrap_or_default()
                .into_iter()
                .filter(|d| d.rule == meta.name)
            {
                diagnostics.push(json!({
                    "message": d.message,
                    "line": byte_offset_to_line(&src, d.span.start),
                    "column": byte_offset_to_col(&src, d.span.start),
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
            "group": meta.group,
            "domains": meta.domains,
            "recommended": meta.recommended,
            "crossFile": meta.cross_file,
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

    // ── domains.json ──
    let domains: Vec<Value> = DOMAINS
        .iter()
        .map(|domain| {
            let mut rules: Vec<&str> = metas
                .iter()
                .filter(|m| m.domains.contains(domain))
                .map(|m| m.name)
                .collect();
            rules.sort_unstable();
            json!({ "name": domain, "rules": rules })
        })
        .collect();

    // ── write ──
    let data_dir = root.join("website/src/data");
    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("error: cannot create {}: {e}", data_dir.display());
        std::process::exit(1);
    }
    let rules_path = data_dir.join("rules.json");
    let domains_path = data_dir.join("domains.json");
    let cli_path = data_dir.join("cli.json");
    let config_ref_path = data_dir.join("config-reference.json");

    let rules_json =
        serde_json::to_string_pretty(&Value::Array(records.clone())).expect("serialize rules.json");
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
        metas.iter().filter(|m| m.cross_file).count()
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
