use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("codegen") => codegen(&args[1..]),
        Some("validate-rules") => validate_rules(&args[1..]),
        Some("perf-lock") => perf_lock(&args[1..]),
        _ => {
            eprintln!("Usage: cargo xtask <task>");
            eprintln!("Available tasks:");
            eprintln!("  codegen         Generate rule implementation + fixture stubs");
            eprintln!("  validate-rules  Validate rule implementations against golden corpus");
            eprintln!(
                "  perf-lock       Enforce the M6 performance lock (<1000ms on jfit mobile lib)"
            );
            eprintln!();
            eprintln!("codegen usage:");
            eprintln!(
                "  cargo xtask codegen rule --name <snake_name> --module <dart_code_linter|pyramid_lint> [--rule-id <id>]"
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

/// Generate a rule implementation stub plus corpus fixture skeletons.
///
/// `cargo xtask codegen rule --name avoid_foo --module dart_code_linter [--rule-id avoid-foo]`
fn codegen(args: &[String]) {
    if args.first().map(|s| s.as_str()) != Some("rule") {
        eprintln!(
            "Usage: cargo xtask codegen rule --name <snake_name> --module <dart_code_linter|pyramid_lint> [--rule-id <id>]"
        );
        std::process::exit(1);
    }

    let mut name: Option<String> = None;
    let mut module: Option<String> = None;
    let mut rule_id: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                i += 1;
                name = args.get(i).cloned();
            }
            "--module" => {
                i += 1;
                module = args.get(i).cloned();
            }
            "--rule-id" => {
                i += 1;
                rule_id = args.get(i).cloned();
            }
            other => {
                eprintln!("error: unknown codegen flag: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let Some(name) = name else {
        eprintln!("error: --name is required");
        std::process::exit(1);
    };
    let Some(module) = module else {
        eprintln!("error: --module is required");
        std::process::exit(1);
    };
    if module != "dart_code_linter" && module != "pyramid_lint" {
        eprintln!("error: --module must be dart_code_linter or pyramid_lint");
        std::process::exit(1);
    }
    // dart_code_linter rule ids are kebab-case; pyramid_lint ids are snake_case.
    let rule_id = rule_id.unwrap_or_else(|| {
        if module == "dart_code_linter" {
            name.replace('_', "-")
        } else {
            name.clone()
        }
    });

    let root = workspace_root();
    let rule_file = root.join(format!("crates/falcon_rules/src/{}/{}.rs", module, name));
    let corpus_dir = root.join(format!("crates/falcon_rules/tests/corpus/{}", rule_id));

    if rule_file.exists() {
        eprintln!("error: {} already exists", rule_file.display());
        std::process::exit(1);
    }

    let struct_name = pascal_case(&name);
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
        "  1. add `pub mod {};` to crates/falcon_rules/src/{}{}",
        name,
        module,
        if module == "pyramid_lint" {
            "/mod.rs"
        } else {
            ".rs"
        }
    );
    println!(
        "  2. register `Box::new({}::{}::{})` in all_rules() (crates/falcon_rules/src/lib.rs)",
        module, name, struct_name
    );
    println!(
        "  3. implement the rule, fill in fixtures, then run `cargo xtask validate-rules --rule {}`",
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
