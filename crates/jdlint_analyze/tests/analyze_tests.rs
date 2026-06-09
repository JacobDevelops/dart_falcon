use jdlint_analyze::{AnalyzeContext, Rule, RuleRegistry};
use jdlint_config::JdlintConfig;
use jdlint_diagnostics::{Diagnostic, Severity, Span};
use jdlint_syntax::Program;
use std::path::Path;

// Mock rules for testing

struct NoOpRule;
impl Rule for NoOpRule {
    fn name(&self) -> &'static str {
        "no_op"
    }
    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        vec![]
    }
}

struct AlwaysErrorRule;
impl Rule for AlwaysErrorRule {
    fn name(&self) -> &'static str {
        "always_error"
    }
    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        vec![Diagnostic::new(
            "always_error",
            Severity::Error,
            "test error",
            ctx.file_path.to_string_lossy().to_string(),
            Span { start: 0, end: 0 },
        )]
    }
}

struct CountFunctionDeclsRule(u32);
impl Rule for CountFunctionDeclsRule {
    fn name(&self) -> &'static str {
        "count_functions"
    }
    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        (0..self.0)
            .map(|i| {
                Diagnostic::new(
                    "count_functions",
                    Severity::Info,
                    format!("function {}", i),
                    "test.dart",
                    Span { start: 0, end: 1 },
                )
            })
            .collect()
    }
}

#[test]
fn test_mock_rule_name() {
    let rule = NoOpRule;
    assert_eq!(rule.name(), "no_op");

    let error_rule = AlwaysErrorRule;
    assert_eq!(error_rule.name(), "always_error");
}

#[test]
fn test_mock_rule_analyze_empty() {
    let rule = NoOpRule;
    let (program, _) = jdlint_dart_parser::parse("void main() {}");
    let config = JdlintConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source: "void main() {}",
        config: &config,
    };

    let diagnostics = rule.analyze(&program, &ctx);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_registry_register_and_run_all_empty() {
    let mut registry = RuleRegistry::new();
    registry.register(Box::new(NoOpRule));

    let (program, _) = jdlint_dart_parser::parse("void main() {}");
    let config = JdlintConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source: "void main() {}",
        config: &config,
    };

    let diagnostics = registry.run_all(&program, &ctx);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_registry_run_all_emits_diagnostic() {
    let mut registry = RuleRegistry::new();
    registry.register(Box::new(AlwaysErrorRule));

    let (program, _) = jdlint_dart_parser::parse("void main() {}");
    let config = JdlintConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source: "void main() {}",
        config: &config,
    };

    let diagnostics = registry.run_all(&program, &ctx);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "always_error");
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn test_registry_multiple_rules() {
    let mut registry = RuleRegistry::new();
    registry.register(Box::new(NoOpRule));
    registry.register(Box::new(AlwaysErrorRule));
    registry.register(Box::new(CountFunctionDeclsRule(2)));

    let (program, _) = jdlint_dart_parser::parse("void main() {}");
    let config = JdlintConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source: "void main() {}",
        config: &config,
    };

    let diagnostics = registry.run_all(&program, &ctx);
    // NoOpRule: 0, AlwaysErrorRule: 1, CountFunctionDeclsRule: 2
    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn test_analyze_context_fields() {
    let config = JdlintConfig::default();
    let source = "void main() {}";
    let path = Path::new("test.dart");

    let ctx = AnalyzeContext {
        file_path: path,
        source,
        config: &config,
    };

    assert_eq!(ctx.file_path, path);
    assert_eq!(ctx.source, source);
    assert_eq!(ctx.file_path.to_string_lossy(), "test.dart");
}

#[test]
fn test_registry_rules_accessor() {
    let mut registry = RuleRegistry::new();
    registry.register(Box::new(NoOpRule));
    registry.register(Box::new(AlwaysErrorRule));

    let rules = registry.rules();
    assert_eq!(rules.len(), 2);
}

#[test]
fn test_analyze_parallel_empty_files() {
    use jdlint_analyze::analyze_parallel;

    let registry = RuleRegistry::new();
    let config = JdlintConfig::default();
    let files: Vec<(std::path::PathBuf, String)> = vec![];

    let diagnostics = analyze_parallel(&registry, &files, &config);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_analyze_parallel_single_file() {
    use jdlint_analyze::analyze_parallel;

    let mut registry = RuleRegistry::new();
    registry.register(Box::new(CountFunctionDeclsRule(1)));

    let config = JdlintConfig::default();
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
    use jdlint_analyze::analyze_parallel;
    use std::path::PathBuf;

    let mut registry = RuleRegistry::new();
    registry.register(Box::new(CountFunctionDeclsRule(1)));

    let config = JdlintConfig::default();
    let files: Vec<(PathBuf, String)> = (0..10)
        .map(|i| {
            (
                PathBuf::from(format!("file_{}.dart", i)),
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
    use jdlint_analyze::analyze_parallel;
    use std::path::PathBuf;

    let mut registry = RuleRegistry::new();
    registry.register(Box::new(AlwaysErrorRule));

    let config = JdlintConfig::default();
    let files: Vec<(PathBuf, String)> = (0..5)
        .map(|i| {
            (
                PathBuf::from(format!("test_{}.dart", i)),
                "class Foo {}".to_string(),
            )
        })
        .collect();

    let diagnostics = analyze_parallel(&registry, &files, &config);
    assert_eq!(diagnostics.len(), 5);
    // Each diagnostic file_path should correspond to one of our input files
    for diag in &diagnostics {
        assert!(
            diag.file_path.starts_with("test_") && diag.file_path.ends_with(".dart"),
            "unexpected file_path: {}",
            diag.file_path
        );
    }
}

#[test]
fn test_end_to_end_parse_analyze_serialize() {
    // Parse a Dart snippet, run a mock rule via RuleRegistry, collect diagnostics, serialize JSON.
    let source = r#"
        class Foo {
            void bar() {}
            void baz() {}
        }
    "#;

    let (program, parse_errors) = jdlint_dart_parser::parse(source);
    assert!(
        parse_errors.is_empty(),
        "unexpected parse errors: {:?}",
        parse_errors
    );

    let mut registry = RuleRegistry::new();
    registry.register(Box::new(AlwaysErrorRule));

    let config = JdlintConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("foo.dart"),
        source,
        config: &config,
    };

    let diagnostics = registry.run_all(&program, &ctx);
    assert_eq!(
        diagnostics.len(),
        1,
        "expected one diagnostic from AlwaysErrorRule"
    );

    let json_value = diagnostics[0].format_json();
    assert_eq!(json_value["rule"], "always_error");
    assert_eq!(json_value["severity"], "Error");
    assert!(json_value["file_path"].as_str().is_some());
}
