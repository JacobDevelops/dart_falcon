//! In-process golden-corpus validation.
//!
//! Runs every rule against its `tests/corpus/<rule-name>/` fixtures and asserts the
//! emitted diagnostics line up with the `/* expect: <rule-name> */` annotations. This is
//! the `cargo test` counterpart of the out-of-process `cargo xtask validate-rules` harness:
//! it needs no pre-built binary, so `cargo test -p falcon_rules` exercises every rule.
//! Fixtures that need SDK member declarations opt into the shared parsed SDK
//! below; other resolved fixtures load local dependencies without an SDK.
//!
//! Invariants enforced:
//!   * every registered rule has real positive and negative corpus coverage;
//!   * every corpus subdirectory maps to a registered rule (no orphan fixtures);
//!   * every `/* expect: */` annotation is matched by a diagnostic on the same line;
//!   * every `msg:` annotation exactly matches the produced diagnostic message;
//!   * recursive ordinary fixtures and grouped project fixtures are covered;
//!   * produced messages end in full stops and use backticks for code spellings;
//!   * no diagnostic fires on a line without a matching annotation (no false positives);
//!   * clean files (zero annotations) produce zero diagnostics;
//!   * source and corpus fixtures contain no explicit stub/placeholder markers.

#[path = "support/corpus.rs"]
mod corpus;
#[path = "support/inference.rs"]
mod inference;
mod support;
use corpus::{
    Expectation, contains_placeholder_marker, cross_file_project_roots, parse_expectations,
};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use falcon_analyze::{
    AnalyzeContext, CrossFileRuleRegistry, ExternalLibraries, ExternalSources, IdentityIndex,
    IdentitySource, LibrarySource, ParsedFile, ProjectFile, Rule, RuleAnalysis, SignatureIndex,
    TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parser::parse;

use falcon_rules::{all_cross_file_rules, all_rules, rule_requires_resolution};

/// Rules registered as deliberate stubs, exempt from the registration coverage
/// check. Each entry must retain a `Stub registration` source marker and a
/// placeholder fixture; executable cross-file projects still need expectations.
const KNOWN_STUB_RULES: &[&str] = &[];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// 1-indexed line number for a byte offset (mirrors the xtask harness).
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

#[derive(Debug)]
struct CorpusDiagnostic {
    rule: String,
    line: usize,
    message: String,
}

fn compare_diagnostics(
    file: &Path,
    expectations: &[Expectation],
    diagnostics: Vec<CorpusDiagnostic>,
    failures: &mut Vec<String>,
) {
    for diagnostic in &diagnostics {
        for problem in message_style_errors(&diagnostic.message) {
            failures.push(format!(
                "STYLE {}:{} — `{}`: {problem}: {:?}",
                file.display(),
                diagnostic.line,
                diagnostic.rule,
                diagnostic.message
            ));
        }
    }
    let mut remaining: Vec<_> = diagnostics.iter().collect();
    let mut unmatched = Vec::with_capacity(expectations.len());
    // Match message-bearing expectations first, so an unconstrained annotation
    // cannot consume the only diagnostic satisfying a constrained one.
    for expectation in expectations
        .iter()
        .filter(|e| e.expected_msg.is_some())
        .chain(expectations.iter().filter(|e| e.expected_msg.is_none()))
    {
        let same_location =
            |d: &&CorpusDiagnostic| d.rule == expectation.rule && d.line == expectation.line;
        let position = remaining.iter().position(|d| {
            same_location(d)
                && expectation
                    .expected_msg
                    .as_ref()
                    .is_none_or(|message| message == &d.message)
        });
        if let Some(position) = position {
            remaining.remove(position);
        } else {
            unmatched.push(expectation);
        }
    }
    // Only diagnose mismatches after every successful match has claimed its
    // diagnostic; an early stale message must not steal a later exact match.
    for expectation in unmatched {
        if let Some(position) = remaining.iter().position(|diagnostic| {
            diagnostic.rule == expectation.rule && diagnostic.line == expectation.line
        }) {
            let diagnostic = remaining.remove(position);
            failures.push(format!(
                "MESSAGE {}:{} — `{}`\n  expected: {:?}\n    actual: {:?}",
                file.display(),
                expectation.line,
                expectation.rule,
                expectation.expected_msg.as_deref().unwrap(),
                diagnostic.message
            ));
        } else {
            failures.push(format!(
                "MISS  {}:{} — `{}` expected but not emitted{}",
                file.display(),
                expectation.line,
                expectation.rule,
                expectation
                    .expected_msg
                    .as_ref()
                    .map_or_else(String::new, |message| format!(": {message:?}"))
            ));
        }
    }
    for diagnostic in remaining {
        failures.push(format!(
            "EXTRA {}:{} — `{}` fired without a matching annotation: {:?}",
            file.display(),
            diagnostic.line,
            diagnostic.rule,
            diagnostic.message
        ));
    }
}

fn message_style_errors(message: &str) -> Vec<&'static str> {
    let mut errors = Vec::new();
    if !message.ends_with('.') {
        errors.push("end the sentence with a full stop");
    }
    if message.bytes().filter(|&byte| byte == b'`').count() % 2 != 0 {
        errors.push("balance code backticks");
    }
    // Check unambiguous code spellings, not apostrophes in contractions or
    // possessives, nor ordinary quoted prose. Backticked strings may contain
    // any Dart spelling, including quotes and punctuation.
    static QUOTED_CODE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"['"](?:[A-Za-z_$][A-Za-z0-9_$]*[.(<][^'"]*|[A-Za-z_$]+_[A-Za-z0-9_$]+|[a-z_$]+[A-Z][A-Za-z0-9_$]*|[A-Z][a-z0-9]+[A-Z][A-Za-z0-9_$]*|as|async|await|bool|break|case|catch|class|const|continue|covariant|default|deferred|do|double|dynamic|else|enum|extends|external|factory|false|final|finally|for|Function|get|if|implements|import|in|int|interface|is|late|mixin|new|null|on|operator|part|required|rethrow|return|set|static|String|super|switch|sync|this|throw|true|try|typedef|var|void|while|with|yield|==|!=|=>|\?\?|\?\.|&&|\|\|)['"]"#).unwrap()
    });
    static BARE_CODE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"\b(?:[a-z][a-z0-9]*[A-Z][A-Za-z0-9]*|[A-Z][a-z0-9]+[A-Z][A-Za-z0-9]*|[A-Za-z_$][A-Za-z0-9_$]*\.[A-Za-z_$][A-Za-z0-9_$]*|[A-Za-z_$]*_[A-Za-z0-9_$]+)\b|&&|\|\||==|!=|=>|\?\?|\?\.",
        ).unwrap()
    });
    if message.split('`').step_by(2).any(|prose| {
        BARE_CODE.find_iter(prose).any(|code| {
            // Conventional prose abbreviations are not qualified Dart names.
            !matches!(code.as_str(), "e.g" | "i.e" | "JavaScript" | "TypeScript")
        })
    }) {
        errors.push("enclose code identifiers, qualified names, and operators in backticks");
    }
    if message.split('`').step_by(2).any(|prose| {
        QUOTED_CODE.find_iter(prose).any(|code| {
            !matches!(
                code.as_str(),
                "'JavaScript'" | "\"JavaScript\"" | "'TypeScript'" | "\"TypeScript\""
            )
        })
    }) {
        errors.push("enclose code in backticks, not quotes");
    }
    errors
}

#[test]
fn message_style_preserves_natural_apostrophes_and_quoted_prose() {
    for message in [
        "Don't repeat the caller's work.",
        "The callers' results must agree.",
        "Prefer 'ordinary prose' here.",
        "JavaScript differs from Flutter and Riverpod, e.g. in browser usage.",
        "The 'Important' label is ordinary prose.",
        "The 'JavaScript' language has different conventions.",
        "Use `Foo('value')` instead.",
        "Use `snake_case` and `&&` instead.",
    ] {
        assert!(message_style_errors(message).is_empty(), "{message}");
    }
    for message in [
        "Missing punctuation",
        "Use `Foo instead.",
        "Use 'var' instead.",
        "Use 'foo.bar()' instead.",
        "Use 'snake_case' instead.",
        "Use snake_case instead.",
        "Merge the conditions using &&.",
    ] {
        assert!(!message_style_errors(message).is_empty(), "{message}");
    }
}

/// Inspect ordinary literals passed directly to `diagnostic`, including inline
/// `format!` templates. Computed messages remain covered by corpus execution.
fn diagnostic_message_literals(source: &str) -> Vec<String> {
    static CALLS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"(?s)\.diagnostic\s*\(\s*(format!\s*\(\s*)?("(?:\\.|[^"\\])*")"#)
            .unwrap()
    });
    static CONTINUATION: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\\\r?\n\s*").unwrap());
    static FORMAT_FIELDS: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\{\{|\}\}|\{[^{}]*\}").unwrap());
    CALLS
        .captures_iter(source)
        .filter_map(|capture| {
            let literal = CONTINUATION.replace_all(&capture[2], "");
            // Rust-only escapes need a Rust literal parser. Leave those literals
            // to corpus execution rather than rejecting valid Rust source.
            let message: String = serde_json::from_str(&literal).ok()?;
            Some(if capture.get(1).is_some() {
                FORMAT_FIELDS
                    .replace_all(&message, |field: &regex::Captures<'_>| match &field[0] {
                        "{{" => "{",
                        "}}" => "}",
                        _ => "value",
                    })
                    .into_owned()
            } else {
                message
            })
        })
        .collect()
}

#[test]
fn static_diagnostic_literals_follow_message_style() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut failures = Vec::new();
    for file in support::files(&root, usize::MAX, Some("rs")) {
        let source = fs::read_to_string(&file).unwrap();
        for message in diagnostic_message_literals(&source) {
            for problem in message_style_errors(&message) {
                failures.push(format!("{}: {problem}: {message:?}", file.display()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn static_message_scan_covers_direct_and_formatted_literals() {
    let source = r#"
        let unrelated = format!("Not a diagnostic");
        ctx.diagnostic("Use `print`.", span);
        self.ctx.diagnostic(format!(
            "Use `{name}` instead of \
             repeating \"ordinary prose\"."
        ), span);
        ctx.diagnostic(format!("Function has {line_count} lines; use `{{...}}`."), span);
        ctx.diagnostic(computed_message, span);
        ctx.diagnostic("\u{41} Rust-only escape.", span);
    "#;
    assert_eq!(
        diagnostic_message_literals(source),
        [
            "Use `print`.",
            "Use `value` instead of repeating \"ordinary prose\".",
            "Function has value lines; use `{...}`.",
        ]
    );
}

#[test]
fn message_expectations_match_duplicates_and_report_actual_text() {
    let expectations =
        parse_expectations("x /* expect: sample */ /* expect: sample, msg: \"Use `Foo`.\" */");
    let diagnostic = |message: &str| CorpusDiagnostic {
        rule: "sample".to_string(),
        line: 1,
        message: message.to_string(),
    };
    let mut failures = Vec::new();
    compare_diagnostics(
        Path::new("nested/example.dart"),
        &expectations,
        vec![diagnostic("Use `Foo`."), diagnostic("Another message.")],
        &mut failures,
    );
    assert!(failures.is_empty(), "{failures:?}");
    compare_diagnostics(
        Path::new("nested/example.dart"),
        &expectations[1..],
        vec![diagnostic("Use `Bar`.")],
        &mut failures,
    );
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("MESSAGE nested/example.dart:1"));
    assert!(failures[0].contains("expected: \"Use `Foo`.\""));
    assert!(failures[0].contains("actual: \"Use `Bar`.\""));
}

#[test]
fn stale_message_does_not_consume_a_later_exact_match() {
    let expectations = parse_expectations(
        "x /* expect: sample, msg: \"Stale message.\" */ /* expect: sample, msg: \"Actual message.\" */",
    );
    let mut failures = Vec::new();
    compare_diagnostics(
        Path::new("bad.dart"),
        &expectations,
        vec![CorpusDiagnostic {
            rule: "sample".to_owned(),
            line: 1,
            message: "Actual message.".to_owned(),
        }],
        &mut failures,
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with("MISS "), "{failures:?}");
    assert!(failures[0].contains("Stale message."));
}

#[test]
fn produced_diagnostics_require_backticks_for_bare_code() {
    for spelling in ["initState", "GestureDetector", "Future.wait", "onPressed"] {
        let mut failures = Vec::new();
        let message = format!("Use {spelling} instead.");
        let expectations = parse_expectations("x /* expect: sample */");
        compare_diagnostics(
            Path::new("bad.dart"),
            &expectations,
            vec![CorpusDiagnostic {
                rule: "sample".to_owned(),
                line: 1,
                message,
            }],
            &mut failures,
        );
        assert!(
            failures.iter().any(|failure| failure.starts_with("STYLE ")),
            "{spelling}: {failures:?}"
        );
        assert!(message_style_errors(&format!("Use `{spelling}` instead.")).is_empty());
    }
}

/// Run a single rule over `source`, retaining messages for golden comparison.
fn run_rule(
    rule_name: &'static str,
    rule: &dyn Rule,
    file: &Path,
    source: &str,
    config: &FalconConfig,
) -> Vec<CorpusDiagnostic> {
    let diagnostics = if inference::requires_sdk(file) {
        inference::analyze(rule_name, rule, file, source, config)
    } else {
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let resolver_dependent = rule_requires_resolution(rule_name);
        let environment =
            falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([file], |_| None);
        let externals = resolver_dependent.then(|| {
            ExternalLibraries::load(&environment, &[(file, program, source)], |path| {
                path == file
            })
        });
        let identity_sources = [IdentitySource {
            path: file,
            parsed: &parsed,
            has_parse_errors: !errors.is_empty(),
        }];
        let identities = resolver_dependent.then(|| {
            IdentityIndex::build(
                &identity_sources,
                &[source],
                &[],
                externals.as_ref().map(|libraries| ExternalSources {
                    libraries,
                    environment: &environment,
                }),
            )
        });
        let types = identities.as_ref().map(|identities| {
            let mut types = TypeIndex::from_resolved_files(
                [(
                    file,
                    LibrarySource {
                        program,
                        has_parse_errors: !errors.is_empty(),
                        has_unresolved_parts: false,
                    },
                )],
                identities,
            );
            if let Some(externals) = externals.as_ref() {
                types.add_external(externals, identities);
            }
            types
        });
        let signatures = identities
            .as_ref()
            .zip(types.as_ref())
            .map(|(identities, types)| {
                SignatureIndex::build(
                    &[(file.to_path_buf(), Arc::clone(&parsed))],
                    externals.as_ref(),
                    identities,
                    types,
                )
            });
        let mut ctx =
            AnalyzeContext::new(file, config, &parsed).with_environment(Some(&environment));
        if let Some(types) = types.as_ref() {
            ctx = ctx.with_types(types);
        }
        if let Some(identities) = identities.as_ref() {
            ctx = ctx.with_identities(identities);
        }
        if let Some(signatures) = signatures.as_ref() {
            ctx = ctx.with_signatures(signatures);
        }
        rule.analyze(rule_name, program, &ctx)
    };
    diagnostics
        .into_iter()
        .map(|d| CorpusDiagnostic {
            rule: d.rule.to_string(),
            line: byte_offset_to_line(source, d.span.start),
            message: d.message.to_string(),
        })
        .collect()
}

/// Load the per-rule corpus config (`corpus/<rule>/config.json`, full falcon.json
/// shape) if present; config-gated rules like use-design-system-item need one to
/// fire at all. Falls back to the default config.
fn corpus_config(rule_dir: &Path) -> FalconConfig {
    let path = rule_dir.join("config.json");
    if path.exists() {
        falcon_rules::load_config(&path)
            .unwrap_or_else(|e| panic!("invalid corpus config {}: {e}", path.display()))
            .config
    } else {
        FalconConfig::default()
    }
}

fn has_rule_expectation(source: &str, rule_name: &str) -> bool {
    parse_expectations(source)
        .iter()
        .any(|expectation| expectation.rule == rule_name)
}

fn is_intentional_clean_fixture(rule_dir: &Path, path: &Path, source: &str) -> bool {
    path.parent() == Some(rule_dir)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("good") || name.starts_with("clean"))
        && parse_expectations(source).is_empty()
}

fn source_rule_name(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.replace('_', "-"))
}

fn corpus_rule_name<'a>(corpus: &Path, path: &'a Path) -> Option<&'a str> {
    path.strip_prefix(corpus)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()
}

#[test]
fn registered_rules_have_real_corpus_coverage() {
    let corpus = corpus_dir();
    let rules = all_rules();
    let cross_file_rules = all_cross_file_rules();
    let cross_file: HashSet<&str> = cross_file_rules.iter().map(|(name, _)| *name).collect();
    let known_stubs: HashSet<&str> = KNOWN_STUB_RULES.iter().copied().collect();
    let registered: HashSet<&str> = rules
        .iter()
        .map(|(name, _)| *name)
        .chain(cross_file.iter().copied())
        .collect();
    let mut failures = Vec::new();

    for rule_name in &registered {
        if known_stubs.contains(rule_name) {
            continue;
        }
        let rule_dir = corpus.join(rule_name);
        if !rule_dir.is_dir() {
            failures.push(format!("MISSING `{rule_name}` corpus directory"));
            continue;
        }

        if cross_file.contains(rule_name) {
            let project_dir = rule_dir.join("cross-file");
            if !project_dir.is_dir() {
                failures.push(format!("MISSING `{rule_name}` cross-file project fixture"));
                continue;
            }
            let projects = cross_file_project_roots(&project_dir);
            let mut has_positive_project = false;
            let mut has_clean_project = false;
            for project in projects {
                let project_name = project
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                let sources: Vec<String> = support::files(&project, usize::MAX, None)
                    .iter()
                    .filter_map(|path| fs::read_to_string(path).ok())
                    .collect();
                if project_name.starts_with("bad")
                    && sources
                        .iter()
                        .any(|source| has_rule_expectation(source, rule_name))
                {
                    has_positive_project = true;
                }
                if project_name.starts_with("good")
                    && sources
                        .iter()
                        .all(|source| parse_expectations(source).is_empty())
                {
                    has_clean_project = true;
                }
            }
            if !has_positive_project {
                failures.push(format!(
                    "NO POSITIVE `{rule_name}` bad cross-file project has an expected diagnostic"
                ));
            }
            if !has_clean_project {
                failures.push(format!("NO CLEAN `{rule_name}` good cross-file project"));
            }
        } else {
            let files = support::files(&rule_dir, usize::MAX, Some("dart"));
            let sources: Vec<(PathBuf, String)> = files
                .into_iter()
                .map(|path| {
                    let source = fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                    (path, source)
                })
                .collect();
            if !sources.iter().any(|(path, source)| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("bad"))
                    && has_rule_expectation(source, rule_name)
            }) {
                failures.push(format!(
                    "NO POSITIVE `{rule_name}` bad fixture has an expected diagnostic"
                ));
            }
            if !sources
                .iter()
                .any(|(path, source)| is_intentional_clean_fixture(&rule_dir, path, source))
            {
                failures.push(format!(
                    "NO CLEAN `{rule_name}` top-level good/clean Dart fixture"
                ));
            }
        }
    }

    let mut corpus_dirs: Vec<PathBuf> = fs::read_dir(&corpus)
        .expect("corpus dir must exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    corpus_dirs.sort();
    for dir in corpus_dirs {
        let name = dir.file_name().unwrap().to_string_lossy();
        if !registered.contains(name.as_ref()) {
            failures.push(format!(
                "ORPHAN corpus dir `{name}` maps to no registered rule"
            ));
        }
    }

    failures.sort();
    assert!(
        failures.is_empty(),
        "rule corpus coverage invariants failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn registered_rules_have_no_unlisted_stub_source_markers() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed: HashSet<&str> = KNOWN_STUB_RULES.iter().copied().collect();
    let mut found = HashSet::new();
    let mut failures = Vec::new();
    for path in support::files(&source_root, usize::MAX, Some("rs")) {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if source.contains("Stub registration") {
            let rule_name = source_rule_name(&path)
                .unwrap_or_else(|| panic!("stub source has no rule name: {}", path.display()));
            if !allowed.contains(rule_name.as_str()) {
                failures.push(format!(
                    "UNLISTED stub marker for `{rule_name}` in {}",
                    path.display()
                ));
            }
            found.insert(rule_name);
        }
    }

    for rule_name in KNOWN_STUB_RULES {
        if !found.contains(*rule_name) {
            failures.push(format!(
                "STALE `{rule_name}` stub allowlist entry has no source marker"
            ));
        }
    }
    failures.sort();
    assert!(
        failures.is_empty(),
        "stub source allowlist is out of sync:\n{}",
        failures.join("\n")
    );
}

#[test]
fn corpus_has_no_unlisted_placeholder_text() {
    let corpus = corpus_dir();
    let allowed: HashSet<&str> = KNOWN_STUB_RULES.iter().copied().collect();
    let mut found = HashSet::new();
    let mut failures = Vec::new();
    for path in support::files(&corpus, usize::MAX, None) {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        if contains_placeholder_marker(&source) {
            let rule_name = corpus_rule_name(&corpus, &path)
                .unwrap_or_else(|| panic!("corpus file has no rule directory: {}", path.display()));
            if !allowed.contains(rule_name) {
                failures.push(format!(
                    "UNLISTED placeholder fixture for `{rule_name}` in {}",
                    path.display()
                ));
            }
            found.insert(rule_name.to_string());
        }
    }

    for rule_name in KNOWN_STUB_RULES {
        if !found.contains(*rule_name) {
            failures.push(format!(
                "STALE `{rule_name}` stub allowlist entry has no placeholder fixture"
            ));
        }
    }
    failures.sort();
    assert!(
        failures.is_empty(),
        "placeholder fixture allowlist is out of sync:\n{}",
        failures.join("\n")
    );
}

#[test]
fn intentional_clean_fixture_must_be_named_and_top_level() {
    let rule_dir = Path::new("/corpus/sample-rule");

    assert!(is_intentional_clean_fixture(
        rule_dir,
        &rule_dir.join("good.dart"),
        "void good() {}",
    ));
    assert!(is_intentional_clean_fixture(
        rule_dir,
        &rule_dir.join("clean_regression.dart"),
        "void clean() {}",
    ));
    assert!(!is_intentional_clean_fixture(
        rule_dir,
        &rule_dir.join("helper.dart"),
        "void helper() {}",
    ));
    assert!(!is_intentional_clean_fixture(
        rule_dir,
        &rule_dir.join("project/good.dart"),
        "void support() {}",
    ));
    assert!(!is_intentional_clean_fixture(
        rule_dir,
        &rule_dir.join("good.dart"),
        "bad(); /* expect: sample-rule */",
    ));
}

#[test]
fn corpus_marker_detection_is_specific() {
    assert!(contains_placeholder_marker(
        "// Placeholder fixture: filled in when the rule is implemented."
    ));
    assert!(contains_placeholder_marker(
        "// Placeholder: compliant code, no annotations."
    ));
    assert!(contains_placeholder_marker(
        "// Placeholder: violation examples with expect annotations will be added"
    ));
    assert!(!contains_placeholder_marker("void placeholderBuilder() {}"));
    assert!(!contains_placeholder_marker(
        "// Placeholder: legitimate example text."
    ));
    assert!(!contains_placeholder_marker("void main() {}"));
    assert!(has_rule_expectation(
        "bad(); /* expect: sample-rule */",
        "sample-rule"
    ));
    assert!(!has_rule_expectation(
        "bad(); /* expect: another-rule */",
        "sample-rule"
    ));
}

#[test]
fn corpus_matches_expectations() {
    let rules = all_rules();
    let by_name: HashMap<&str, &dyn Rule> = rules
        .iter()
        .map(|(name, rule)| (*name, rule.as_ref()))
        .collect();

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

    let cross_file_rule_names: std::collections::HashSet<&str> = all_cross_file_rules()
        .iter()
        .map(|(name, _)| *name)
        .collect();

    for dir in &rule_dirs {
        let rule_name = dir.file_name().unwrap().to_string_lossy().to_string();

        // Cross-file rules keep their multi-file fixtures in a `cross-file/`
        // subdirectory and are validated by the dedicated test below.
        if cross_file_rule_names.contains(rule_name.as_str()) {
            continue;
        }

        // Invariant: every corpus directory must correspond to a registered rule.
        let Some((registered_name, rule)) = by_name.get_key_value(rule_name.as_str()) else {
            failures.push(format!(
                "ORPHAN corpus dir `{rule_name}` maps to no registered rule"
            ));
            continue;
        };

        let config = corpus_config(dir);
        for file in corpus::ordinary_fixture_files(dir, rule_requires_resolution(&rule_name)) {
            total_files += 1;
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));

            let expectations: Vec<Expectation> = parse_expectations(&source)
                .into_iter()
                .filter(|e| e.rule == rule_name)
                .collect();
            total_expectations += expectations.len();

            let diagnostics = run_rule(registered_name, *rule, &file, &source, &config);
            compare_diagnostics(&file, &expectations, diagnostics, &mut failures);
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

/// In-process validation of cross-file rules against their multi-file fixtures
/// under `corpus/<rule>/cross-file/`. Each fixture directory is run as one unit;
/// `/* expect: <rule> */` annotations are matched by (file, line).
#[test]
fn cross_file_corpus_matches_expectations() {
    let corpus = corpus_dir();
    let mut failures: Vec<String> = Vec::new();
    let mut total_expectations = 0usize;

    for (rule_name, rule) in all_cross_file_rules() {
        let dir = corpus.join(rule_name).join("cross-file");
        assert!(
            dir.is_dir(),
            "cross-file rule `{}` is missing its corpus/<rule>/cross-file/ fixture",
            rule_name
        );
        let config = corpus_config(&corpus.join(rule_name));
        let mut rule_expectations = 0;
        let mut registry = CrossFileRuleRegistry::new();
        registry.register(rule_name, rule);

        for project in cross_file_project_roots(&dir) {
            let fixture_sources: Vec<(PathBuf, String)> =
                support::files(&project, usize::MAX, None)
                    .into_iter()
                    .filter_map(|path| fs::read_to_string(&path).ok().map(|source| (path, source)))
                    .collect();
            let dart_sources: Vec<_> = fixture_sources
                .iter()
                .filter(|(path, _)| {
                    path.extension().and_then(|extension| extension.to_str()) == Some("dart")
                })
                .cloned()
                .collect();
            let packages = falcon_analyze::package_identities(
                fixture_sources.iter().map(|(path, _)| path.as_path()),
            );
            let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
                fixture_sources.iter().map(|(path, _)| path.as_path()),
                |_| None,
            );
            let (_, mut files, indexes) = falcon_analyze::analyze_sequential_collecting_resolving(
                &falcon_analyze::RuleRegistry::new(),
                &dart_sources,
                &config,
                true,
                rule_requires_resolution(rule_name),
                falcon_analyze::Workspace {
                    packages: &packages,
                    environment: Some(&environment),
                },
            );
            files.extend(
                fixture_sources
                    .iter()
                    .filter(|(path, _)| {
                        path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml")
                    })
                    .map(|(path, source)| ProjectFile {
                        path: path.clone(),
                        parsed: Arc::new(ParsedFile {
                            source: source.clone().into_boxed_str(),
                            tree: falcon_dart_parser::lex_tree(source),
                        }),
                        has_parse_errors: false,
                    }),
            );

            let diagnostics = registry
                .run_all(&environment, &files, &config, indexes.as_ref())
                .expect("cross-file rules thread starts");
            for (path, source) in &fixture_sources {
                let expectations: Vec<_> = parse_expectations(source)
                    .into_iter()
                    .filter(|expectation| expectation.rule == rule_name)
                    .collect();
                total_expectations += expectations.len();
                rule_expectations += expectations.len();
                let found = diagnostics
                    .iter()
                    .filter(|diagnostic| Path::new(&diagnostic.file_path) == path)
                    .map(|diagnostic| CorpusDiagnostic {
                        rule: diagnostic.rule.to_string(),
                        line: byte_offset_to_line(source, diagnostic.span.start),
                        message: diagnostic.message.to_string(),
                    })
                    .collect();
                compare_diagnostics(path, &expectations, found, &mut failures);
            }
            for diagnostic in &diagnostics {
                if !fixture_sources
                    .iter()
                    .any(|(path, _)| Path::new(&diagnostic.file_path) == path)
                {
                    failures.push(format!(
                        "EXTRA {} — diagnostic outside fixture: {:?}",
                        diagnostic.file_path, diagnostic.message
                    ));
                }
            }
        }
        assert!(
            rule_expectations > 0,
            "cross-file rule `{rule_name}` fixture has no expectations"
        );
    }

    assert!(
        total_expectations > 0,
        "cross-file corpus produced no expectations — runner is not exercising fixtures"
    );
    assert!(
        failures.is_empty(),
        "cross-file corpus validation failed:\n{}",
        failures.join("\n")
    );
}

/// Resolver-dependent *per-file* rules validated against multi-file
/// `corpus/<rule>/project/` fixtures: run per file with the real library
/// grouping attached (part siblings + unresolved flag), mirroring the
/// resolving pipeline.
#[test]
fn resolver_dependent_per_file_project_corpus() {
    use falcon_analyze::{LibrarySource, group_libraries, library_unit};
    use falcon_syntax::cst::Program;

    let rules = all_rules();
    let corpus = corpus_dir();
    let mut failures: Vec<String> = Vec::new();
    let mut total_expectations = 0usize;

    for (rule_name, rule) in rules
        .iter()
        .filter(|(name, _)| rule_requires_resolution(name))
    {
        let dir = corpus.join(rule_name).join("project");
        if !dir.is_dir() {
            continue;
        }
        let config = corpus_config(&corpus.join(rule_name));

        let paths = support::files(&dir, usize::MAX, Some("dart"));
        let source_paths = paths.clone();
        let sdk_project = paths
            .iter()
            .any(|path| inference::requires_sdk(path))
            .then(|| inference::sdk_fixture::stage_project(&dir));
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|path| {
                sdk_project.as_ref().map_or_else(
                    || path.clone(),
                    |project| inference::sdk_fixture::staged_file(project.path(), &dir, &path),
                )
            })
            .collect();
        let sources: Vec<String> = paths
            .iter()
            .map(|p| fs::read_to_string(p).unwrap())
            .collect();
        let results: Vec<_> = sources.iter().map(|source| parse(source)).collect();
        let has_parse_errors: Vec<_> = results
            .iter()
            .map(|result| !result.errors.is_empty())
            .collect();
        let parsed: Vec<_> = results
            .into_iter()
            .zip(&sources)
            .map(|(result, source)| {
                Arc::new(ParsedFile {
                    source: source.clone().into_boxed_str(),
                    tree: result.tree,
                })
            })
            .collect();
        let programs: Vec<Program<'_>> = parsed.iter().map(|parsed| parsed.program()).collect();
        let parsed_refs: Vec<_> = parsed.iter().collect();
        let path_prog: Vec<(PathBuf, Program<'_>)> = paths
            .iter()
            .cloned()
            .zip(programs.iter().copied())
            .collect();
        let grouping = group_libraries(&path_prog);
        let identity_sources: Vec<_> = paths
            .iter()
            .zip(&has_parse_errors)
            .zip(&parsed)
            .map(|((path, has_parse_errors), parsed)| IdentitySource {
                path,
                parsed,
                has_parse_errors: *has_parse_errors,
            })
            .collect();
        let source_refs: Vec<&str> = sources.iter().map(String::as_str).collect();
        let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            paths.iter().map(PathBuf::as_path),
            |config| {
                let root = config?.flutter_root.as_ref()?;
                falcon_analyze::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
            },
        );
        let external_inputs: Vec<_> = paths
            .iter()
            .zip(&programs)
            .zip(&source_refs)
            .map(|((path, program), source)| (path.as_path(), *program, *source))
            .collect();
        let externals =
            falcon_analyze::ExternalLibraries::load(&environment, &external_inputs, |path| {
                paths.iter().any(|local| local == path)
            });
        let packages = falcon_analyze::package_identities(paths.iter().map(PathBuf::as_path));
        let identities = IdentityIndex::build(
            &identity_sources,
            &source_refs,
            &packages,
            Some(falcon_analyze::ExternalSources {
                libraries: &externals,
                environment: &environment,
            }),
        );
        let mut type_index = TypeIndex::from_resolved_files(
            has_parse_errors
                .iter()
                .enumerate()
                .map(|(i, has_parse_errors)| {
                    (
                        paths[i].as_path(),
                        LibrarySource {
                            program: programs[i],
                            has_parse_errors: *has_parse_errors,
                            has_unresolved_parts: grouping.is_unresolved(i),
                        },
                    )
                }),
            &identities,
        );
        type_index.add_external(&externals, &identities);
        let signature_files: Vec<_> = paths.iter().cloned().zip(parsed.iter().cloned()).collect();
        let signatures =
            SignatureIndex::build(&signature_files, Some(&externals), &identities, &type_index);

        for (i, path) in paths.iter().enumerate() {
            let unit = library_unit(&grouping, &parsed_refs, i);
            let ctx = AnalyzeContext::new(path, &config, &parsed[i])
                .with_types(&type_index)
                .with_identities(&identities)
                .with_signatures(&signatures)
                .with_library(&unit)
                .with_environment(Some(&environment));
            let diagnostics = rule
                .analyze(rule_name, programs[i], &ctx)
                .into_iter()
                .map(|diagnostic| CorpusDiagnostic {
                    rule: diagnostic.rule.to_string(),
                    line: byte_offset_to_line(&sources[i], diagnostic.span.start),
                    message: diagnostic.message.to_string(),
                })
                .collect();
            let expectations: Vec<_> = parse_expectations(&sources[i])
                .into_iter()
                .filter(|expectation| expectation.rule == *rule_name)
                .collect();
            total_expectations += expectations.len();
            compare_diagnostics(&source_paths[i], &expectations, diagnostics, &mut failures);
        }
    }

    assert!(
        total_expectations > 0,
        "no resolver-dependent per-file project expectations exercised"
    );
    assert!(
        failures.is_empty(),
        "resolver-dependent per-file project corpus failed:\n{}",
        failures.join("\n")
    );
}
