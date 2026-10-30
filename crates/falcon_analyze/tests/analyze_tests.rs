use falcon_analyze::{
    AnalyzeContext, CrossFileRuleAnalysis, ParsedFile, Rule, RuleAnalysis, RulePreparation,
    RuleRegistry,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::cst::Program;
use std::path::Path;
use std::sync::Arc;

fn analyze_parallel(
    registry: &RuleRegistry,
    files: &[(std::path::PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    falcon_analyze::analyze_parallel_collecting_resolving(
        registry,
        files,
        config,
        false,
        false,
        falcon_analyze::Workspace::default(),
    )
    .0
}

// Mock rules for testing

struct NoOpRule;
impl Rule for NoOpRule {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        _ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(vec![])
    }
}

struct AlwaysErrorRule;
impl Rule for AlwaysErrorRule {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(vec![Diagnostic::new(
            "deliberately-wrong-id",
            Severity::Error,
            "test error",
            ctx.file_path().to_string_lossy().to_string(),
            Span { start: 0, end: 0 },
        )])
    }
}

struct CountFunctionDeclsRule(u32);
impl Rule for CountFunctionDeclsRule {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        _ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(
            (0..self.0)
                .map(|i| {
                    Diagnostic::new(
                        "count_functions",
                        Severity::Info,
                        format!("function {i}"),
                        "test.dart",
                        Span { start: 0, end: 1 },
                    )
                })
                .collect(),
        )
    }
}

#[test]
fn test_mock_rule_analyze_empty() {
    let rule = NoOpRule;
    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse("void main() {}");
    let parsed = Arc::new(ParsedFile {
        source: "void main() {}".into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

    let diagnostics = rule.analyze("no_op", parsed.program(), &ctx);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_registry_register_and_run_all_empty() {
    let mut registry = RuleRegistry::new();
    registry.register("no_op", Box::new(NoOpRule));

    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse("void main() {}");
    let parsed = Arc::new(ParsedFile {
        source: "void main() {}".into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

    let diagnostics = registry.run_all(parsed.program(), &ctx);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_registry_run_all_emits_diagnostic() {
    let mut registry = RuleRegistry::new();
    registry.register("always_error", Box::new(AlwaysErrorRule));

    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse("void main() {}");
    let parsed = Arc::new(ParsedFile {
        source: "void main() {}".into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

    let diagnostics = registry.run_all(parsed.program(), &ctx);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "always_error");
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn direct_analysis_uses_provided_registered_name() {
    let source = "void main() {}";
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parse(source).tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

    let diagnostics = AlwaysErrorRule.analyze("registered-name", parsed.program(), &ctx);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "registered-name");
    assert_eq!(diagnostics[0].message, "test error");
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn cross_file_registry_overwrites_incoming_diagnostic_identity() {
    struct WrongIdentityRule;
    impl falcon_analyze::CrossFileRule for WrongIdentityRule {
        fn prepare_project(
            &self,
            _environment: &falcon_analyze::AnalysisEnvironment,
            _files: &[falcon_analyze::ProjectFile],
            _config: &FalconConfig,
            _indexes: Option<&falcon_analyze::SemanticIndexes>,
        ) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                "deliberately-wrong-id",
                Severity::Error,
                "project error",
                "test.dart",
                Span { start: 0, end: 0 },
            )]
        }
    }

    let mut registry = falcon_analyze::CrossFileRuleRegistry::new();
    registry.register("registered-project-name", Box::new(WrongIdentityRule));
    assert_eq!(registry.rules().len(), 1);
    assert_eq!(registry.rules()[0].0, "registered-project-name");
    let diagnostics = registry
        .run_all(
            &falcon_analyze::AnalysisEnvironment::default(),
            &[],
            &FalconConfig::default(),
            None,
        )
        .expect("cross-file rules thread starts");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "registered-project-name");
    assert_eq!(diagnostics[0].message, "project error");

    let direct = WrongIdentityRule.analyze_project(
        "registered-project-name",
        &falcon_analyze::AnalysisEnvironment::default(),
        &[],
        &FalconConfig::default(),
        None,
    );
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].rule, "registered-project-name");
    assert_eq!(direct[0].message, "project error");
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn test_registry_multiple_rules() {
    let mut registry = RuleRegistry::new();
    registry.register("no_op", Box::new(NoOpRule));
    registry.register("always_error", Box::new(AlwaysErrorRule));
    registry.register("count_functions", Box::new(CountFunctionDeclsRule(2)));

    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse("void main() {}");
    let parsed = Arc::new(ParsedFile {
        source: "void main() {}".into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

    let diagnostics = registry.run_all(parsed.program(), &ctx);
    // NoOpRule: 0, AlwaysErrorRule: 1, CountFunctionDeclsRule: 2
    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn test_analyze_context_fields() {
    let config = FalconConfig::default();
    let source = "void main() {}";
    let path = Path::new("test.dart");
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parse(source).tree,
    });

    let ctx = AnalyzeContext::new(path, &config, &parsed);

    assert_eq!(ctx.file_path(), path);
    assert_eq!(ctx.source(), source);
    assert_eq!(ctx.file_path().to_string_lossy(), "test.dart");
}

#[test]
fn test_registry_rules_accessor() {
    let mut registry = RuleRegistry::new();
    registry.register("no_op", Box::new(NoOpRule));
    registry.register("always_error", Box::new(AlwaysErrorRule));

    let rules = registry.rules();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].0, "no_op");
    assert_eq!(rules[1].0, "always_error");
}

#[test]
fn test_analyze_parallel_empty_files() {
    let registry = RuleRegistry::new();
    let config = FalconConfig::default();
    let files: Vec<(std::path::PathBuf, String)> = vec![];

    let diagnostics = analyze_parallel(&registry, &files, &config);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_analyze_parallel_single_file() {
    let mut registry = RuleRegistry::new();
    registry.register("count_functions", Box::new(CountFunctionDeclsRule(1)));

    let config = FalconConfig::default();
    let files = vec![(
        std::path::PathBuf::from("test.dart"),
        "void main() {}".to_string(),
    )];

    let diagnostics = analyze_parallel(&registry, &files, &config);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "count_functions");
}

#[test]
fn test_analyze_parallel_ten_files() {
    use std::path::PathBuf;

    let mut registry = RuleRegistry::new();
    registry.register("count_functions", Box::new(CountFunctionDeclsRule(1)));

    let config = FalconConfig::default();
    let files: Vec<(PathBuf, String)> = (0..10)
        .map(|i| {
            (
                PathBuf::from(format!("file_{i}.dart")),
                "void main() {}".to_string(),
            )
        })
        .collect();

    let diagnostics = analyze_parallel(&registry, &files, &config);
    // CountFunctionDeclsRule(1) emits 1 diagnostic per file × 10 files
    assert_eq!(diagnostics.len(), 10);
}

#[test]
fn test_analyze_parallel_result_correctness_multi_file() {
    use std::path::PathBuf;

    let mut registry = RuleRegistry::new();
    registry.register("always_error", Box::new(AlwaysErrorRule));

    let config = FalconConfig::default();
    let files: Vec<(PathBuf, String)> = (0..5)
        .map(|i| {
            (
                PathBuf::from(format!("test_{i}.dart")),
                "class Foo {}".to_string(),
            )
        })
        .collect();

    let diagnostics = analyze_parallel(&registry, &files, &config);
    assert_eq!(diagnostics.len(), 5);
    // Each diagnostic file_path should correspond to one of our input files
    for diag in &diagnostics {
        assert!(
            diag.file_path.starts_with("test_")
                && std::path::Path::new(&diag.file_path)
                    .extension()
                    .is_some_and(|ext| ext == "dart"),
            "unexpected file_path: {}",
            diag.file_path
        );
    }
}

#[test]
fn test_end_to_end_parse_analyze_serialize() {
    // Parse a Dart snippet, run a mock rule via RuleRegistry, collect diagnostics, serialize JSON.
    let source = r"
        class Foo {
            void bar() {}
            void baz() {}
        }
    ";

    let falcon_dart_parser::ParseResult {
        tree,
        errors: parse_errors,
    } = falcon_dart_parser::parse(source);
    assert!(
        parse_errors.is_empty(),
        "unexpected parse errors: {parse_errors:?}"
    );

    let mut registry = RuleRegistry::new();
    registry.register("always_error", Box::new(AlwaysErrorRule));

    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("foo.dart"), &config, &parsed);

    let diagnostics = registry.run_all(parsed.program(), &ctx);
    assert_eq!(
        diagnostics.len(),
        1,
        "expected one diagnostic from AlwaysErrorRule"
    );

    let json_value = serde_json::to_value(&diagnostics[0]).unwrap();
    assert_eq!(json_value["rule"], "always_error");
    assert_eq!(json_value["severity"], "error");
    assert!(json_value["file_path"].as_str().is_some());
}

#[test]
fn test_context_diagnostic_uses_file_and_warning_default() {
    let config = FalconConfig::default();
    let parsed = Arc::new(ParsedFile {
        source: "abc".into(),
        tree: falcon_dart_parser::parse("abc").tree,
    });
    let ctx = AnalyzeContext::new(Path::new("foo.dart"), &config, &parsed);
    let span = falcon_syntax::cst::Span { start: 1, end: 3 };
    let diagnostic = ctx.diagnostic("message", span);
    assert_eq!(diagnostic.rule, "");
    assert_eq!(diagnostic.message, "message");
    assert_eq!(diagnostic.file_path, "foo.dart");
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.span, Span { start: 1, end: 3 });
}

#[test]
fn context_reuses_per_file_analysis_facts() {
    let source = "int value = 0; void main() { while (value < 2) { value++; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    let config = FalconConfig::default();
    let path = Path::new("test.dart");
    let identities = falcon_analyze::IdentityIndex::from_project_files(
        &[falcon_analyze::IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let ctx = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    assert!(std::ptr::eq(
        ctx.flow_writes(program),
        ctx.flow_writes(program)
    ));
    assert!(std::ptr::eq(ctx.globals(program), ctx.globals(program)));
    assert!(ctx.globals(program).contains_key("value"));
    assert_eq!(
        ctx.tokens()
            .map(falcon_syntax::cst::SyntaxToken::span)
            .collect::<Vec<_>>(),
        ctx.tokens()
            .map(falcon_syntax::cst::SyntaxToken::span)
            .collect::<Vec<_>>(),
    );
    assert!(ctx.tokens().next().is_some());

    let second = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
    assert!(!std::ptr::eq(
        ctx.flow_writes(program),
        second.flow_writes(program)
    ));
}

#[test]
fn context_fix_uses_lossless_cst_byte_spans() {
    use falcon_diagnostics::{FixKind, TextEdit};

    let source = "// café\r\nvoid main() { var value = 1; /* kept */ }\r\n";
    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse(source);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
    let span = ctx
        .tokens()
        .find(|token| token.text() == "var")
        .unwrap()
        .span();
    let diagnostic = ctx.with_fix(
        ctx.diagnostic("Prefer final", span),
        "Replace `var` with `final`",
        FixKind::Safe,
        vec![TextEdit::new(span, "final")],
    );

    assert_eq!(
        diagnostic.fix.unwrap().apply(ctx.source()).unwrap(),
        "// café\r\nvoid main() { final value = 1; /* kept */ }\r\n"
    );
}

#[test]
fn context_rejects_invalid_rule_fixes() {
    use falcon_diagnostics::{FixKind, TextEdit};

    let source = "// é\nvoid main() {}";
    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parse(source);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
    for spans in [
        vec![],
        vec![Span { start: 4, end: 5 }],
        vec![Span {
            start: 0,
            end: source.len() + 1,
        }],
        vec![Span { start: 5, end: 6 }, Span { start: 0, end: 1 }],
        vec![Span { start: 0, end: 6 }, Span { start: 5, end: 6 }],
    ] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ctx.with_fix(
                ctx.diagnostic("Invalid fix", Span { start: 0, end: 0 }),
                "Invalid fix",
                FixKind::Safe,
                spans
                    .into_iter()
                    .map(|span| TextEdit::new(span, ""))
                    .collect(),
            )
        }));
        if cfg!(debug_assertions) {
            assert!(
                result.is_err(),
                "invalid rule fix must panic in debug builds"
            );
        } else {
            assert!(result.unwrap().fix.is_none());
        }
    }
}

#[test]
fn panicking_rules_report_worker_errors_and_leave_the_pool_usable() {
    struct PanickingRule;
    impl Rule for PanickingRule {
        fn prepare<'a>(
            &self,
            _program: Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            assert!(
                ctx.file_path() != Path::new("panic.dart"),
                "deliberate rule panic"
            );
            RulePreparation::Diagnostics(vec![
                ctx.diagnostic("healthy file survives", Span { start: 0, end: 0 }),
            ])
        }
    }

    let mut registry = RuleRegistry::new();
    registry.register("panicking-rule", Box::new(PanickingRule));
    let files = vec![
        ("panic.dart".into(), "void main() {}".into()),
        ("healthy.dart".into(), "void main() {}".into()),
    ];
    let config = FalconConfig::default();
    for resolve in [false, true] {
        for analyze in [
            falcon_analyze::analyze_sequential_collecting_resolving,
            falcon_analyze::analyze_parallel_collecting_resolving,
        ] {
            let (diagnostics, retained, _) = analyze(
                &registry,
                &files,
                &config,
                true,
                resolve,
                falcon_analyze::Workspace::default(),
            );
            assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
            let failure = diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.rule == falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
                })
                .unwrap();
            assert_eq!(failure.severity, Severity::Error);
            assert_eq!(failure.file_path, "panic.dart");
            assert!(failure.message.contains("deliberate rule panic"));
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == "panicking-rule" && diagnostic.message == "healthy file survives"
            }));
            assert_eq!(retained.len(), files.len());
        }
    }
    assert!(analyze_parallel(&RuleRegistry::new(), &files, &config).is_empty());
}

#[test]
fn parallel_rules_run_on_shared_stack_workers_with_and_without_resolution() {
    struct CheckWorker;
    impl Rule for CheckWorker {
        fn prepare<'a>(
            &self,
            _program: Program<'a>,
            _ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            assert!(
                std::thread::current()
                    .name()
                    .unwrap()
                    .starts_with("falcon-stack-")
            );
            let worker = std::thread::current().id();
            assert_eq!(
                falcon_dart_parser::with_stack(|| std::thread::current().id()).unwrap(),
                worker
            );
            RulePreparation::Diagnostics(Vec::new())
        }
    }

    let mut registry = RuleRegistry::new();
    registry.register("check-worker", Box::new(CheckWorker));
    let files: Vec<_> = (0..32)
        .map(|index| (format!("file{index}.dart").into(), "void main() {}".into()))
        .collect();
    for resolve in [false, true] {
        let (diagnostics, retained, _) = falcon_analyze::analyze_parallel_collecting_resolving(
            &registry,
            &files,
            &FalconConfig::default(),
            true,
            resolve,
            falcon_analyze::Workspace::default(),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(retained.len(), files.len());
    }
}
