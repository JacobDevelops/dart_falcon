use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn check_chain(parameter_type: &str, value: &str, operand: &str, operator: &str, form: &str) {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("falcon.json"), "{}\n").unwrap();
    let expression = format!("a{}", format!(" {operator} {operand}").repeat(99_999));
    let declaration = match form {
        "initializer" => format!("final x = {expression}; print(x);"),
        "argument" => format!("consume({expression});"),
        "statement" => format!("{expression};"),
        "top_level" => format!("final x = {expression};"),
        "field" => format!("class C {{ final x = {expression}; }}"),
        _ => unreachable!(),
    };
    let body = if matches!(form, "top_level" | "field") {
        declaration
    } else {
        format!("void f({parameter_type} a) {{ {declaration} }}")
    };
    let source = format!(
        "void marker(Object markerParameter) {{}}\nvoid consume(Object? value) {{ print(value); }}\n{parameter_type} a = {value};\n{body}\n"
    );
    std::fs::write(project.path().join("main.dart"), source).unwrap();
    // Subprocesses use normal stacks independently of the test harness.
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args([
            "check",
            ".",
            "--format",
            "json",
            "--max-errors",
            "1",
            "--error-on-warnings",
            "--exit-code",
            "2",
        ])
        .current_dir(project.path())
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env_remove("RUST_MIN_STACK")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(2)),
        "{form}: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    // A known lint proves file discovery; exit 2 excludes worker and pipeline failures.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("avoid-unused-parameters")
            && stdout.contains("markerParameter")
            && !stdout.contains("rules-thread-error"),
        "{form}: analysis did not complete\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn all_rules_config() -> String {
    let mut groups = std::collections::BTreeMap::<_, Vec<_>>::new();
    for rule in falcon_rules::meta::RULE_METADATA {
        let section = if rule.is_cross_file() {
            "cross-file"
        } else {
            "linter"
        };
        groups
            .entry((section, rule.group.as_str()))
            .or_default()
            .push(format!("\"{}\": \"warn\"", rule.name));
    }
    let section = |name| {
        groups
            .iter()
            .filter(|((section, _), _)| *section == name)
            .map(|((_, group), rules)| format!("\"{group}\": {{{}}}", rules.join(", ")))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{{\"linter\": {{\"rules\": {{{}}}}}, \"cross-file\": {{\"rules\": {{{}}}}}}}",
        section("linter"),
        section("cross-file")
    )
}

#[test]
fn deep_nesting_recovers_in_linear_time() {
    for config in ["{}\n".to_owned(), all_rules_config()] {
        for (open, close) in [("[", "]"), ("(", ")"), ("{", "}"), ("{'key': ", "}")] {
            let project = tempfile::tempdir().unwrap();
            std::fs::write(project.path().join("falcon.json"), &config).unwrap();
            let source = format!(
                "void marker(Object markerParameter) {{}}\nmain() {{ var x = {}0{}; print('okay'); }}\n",
                open.repeat(10_000),
                close.repeat(10_000)
            );
            std::fs::write(project.path().join("main.dart"), source).unwrap();
            let mut child = Command::new(env!("CARGO_BIN_EXE_falcon"))
                .args(["check", ".", "--format", "json", "--exit-code", "2"])
                .current_dir(project.path())
                .env_remove("DART_SDK")
                .env_remove("FLUTTER_ROOT")
                .env_remove("RUST_MIN_STACK")
                .env("PATH", "")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() > deadline {
                    child.kill().unwrap();
                    panic!("{open}: falcon check exceeded ten seconds");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.code() == Some(2)
                    && stdout.matches("\"syntax-error\"").count() == 1
                    && stdout.contains("nesting too deep")
                    && stdout.contains("markerParameter")
                    && !stdout.contains("rules-thread-error"),
                "{open}: {}\nstdout: {stdout}\nstderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }
}

macro_rules! chain_tests {
    ($module:ident, $parameter_type:literal, $value:literal, $operand:literal, $operator:literal; $($form:ident),*) => {
        mod $module {
            $(
                #[test]
                fn $form() {
                    super::check_chain($parameter_type, $value, $operand, $operator, stringify!($form));
                }
            )*
        }
    };
}

chain_tests!(addition, "int", "1", "a", "+"; initializer, argument, statement, top_level, field);
chain_tests!(and, "bool", "true", "a", "&&"; initializer, argument, statement, top_level, field);
chain_tests!(or, "bool", "true", "a", "||"; initializer, argument, statement, top_level, field);
chain_tests!(if_null, "int?", "1", "a", "??"; initializer, argument, statement, top_level, field);
chain_tests!(string, "String", "'x'", "'x'", "+"; initializer, argument, statement, top_level, field);

mod semantic_walkers {
    use std::process::Command;

    pub(super) fn check_rule_source(
        group: &str,
        rule: &str,
        source: &str,
        extra_files: &[(&str, &str)],
    ) -> String {
        let project = tempfile::tempdir().unwrap();
        for (name, contents) in extra_files {
            let path = project.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        let mut rules = serde_json::json!({
            "recommended": false,
            "correctness": {"avoid-unused-parameters": "warn"},
        });
        if rules.get(group).is_none() {
            rules[group] = serde_json::json!({});
        }
        rules[group][rule] = serde_json::json!("warn");
        std::fs::write(
            project.path().join("falcon.json"),
            serde_json::json!({"linter": {"rules": rules}}).to_string(),
        )
        .unwrap();
        let source = format!("{source}\nvoid marker(Object markerParameter) {{}}\n");
        std::fs::write(project.path().join("main.dart"), source).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_falcon"));
        command
            .args(["check", "main.dart", "--format", "json", "--exit-code", "2"])
            .current_dir(project.path())
            .env_remove("DART_SDK")
            .env_remove("FLUTTER_ROOT")
            .env_remove("RUST_MIN_STACK")
            .env("RAYON_NUM_THREADS", "4")
            .env("PATH", "");
        if project.path().join("sdk").is_dir() {
            command.env("DART_SDK", project.path().join("sdk"));
        }
        let output = command.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}\nstdout: {stdout}\nstderr: {stderr}",
            output.status,
        );
        assert!(
            stdout.contains("avoid-unused-parameters")
                && stdout.contains("markerParameter")
                && !stdout.contains("rules-thread-error"),
            "analysis did not complete\nstdout: {stdout}\nstderr: {stderr}",
        );
        stdout.into_owned()
    }

    fn check(operand: &str, operator: &str, template: &str, expect_async_gap: bool) {
        let chain = format!(
            "{operand}{}",
            format!(" {operator} {operand}").repeat(99_999)
        );
        let source = format!(
            "import 'package:flutter/material.dart';\n{}\n",
            template.replace("{chain}", &chain),
        );
        let stdout = check_rule_source(
            "correctness",
            "use-setstate-synchronously",
            &source,
            &[
                (
                    "pubspec.yaml",
                    "name: chain_test\ndependencies:\n  flutter:\n    path: flutter\n",
                ),
                (
                    ".dart_tool/package_config.json",
                    r#"{"configVersion":2,"packages":[{"name":"flutter","rootUri":"../flutter","packageUri":"lib/"}]}"#,
                ),
                (
                    "flutter/lib/material.dart",
                    "class Widget {}\nabstract class State<T extends Widget> {\n  bool get mounted => true;\n  void setState(void Function() callback) { callback(); }\n}\n",
                ),
            ],
        );
        if expect_async_gap {
            assert!(
                stdout.contains("use-setstate-synchronously"),
                "async fallback did not report the gap\nstdout: {stdout}",
            );
        }
    }

    #[test]
    fn parameter_default_syntax() {
        check(
            "1",
            "+",
            "class C { void f([int value = {chain}]) {} }",
            false,
        );
    }

    #[test]
    fn equality_constant_bound_names() {
        check(
            "true",
            "&&",
            "void f() { if (({chain}) == true) {} }",
            false,
        );
    }

    #[test]
    fn async_gap_tree() {
        check(
            "1",
            "+",
            "class C extends State<Widget> { Future<void> f() async { final value = {chain}; await Future.value(); setState(() { print(value); }); } }",
            true,
        );
    }
}

mod guard_walkers {
    use super::semantic_walkers::check_rule_source;
    use std::process::Command;

    #[test]
    fn repeatable_switch_guard() {
        let chain = format!("true{}", " && true".repeat(99_999));
        let source = format!(
            "void f(Object value) {{ switch (value) {{ \
             case int n when {chain}: break; \
             case String s when true: break; \
             case String s when true: break; \
             }} }}"
        );
        let stdout = check_rule_source(
            "correctness",
            "avoid-duplicate-switch-case-conditions",
            &source,
            &[],
        );
        assert!(stdout.contains("avoid-duplicate-switch-case-conditions"));
    }

    #[test]
    fn debug_print_arguments() {
        let chain = format!("1{}", " + 1".repeat(99_999));
        let source = format!(
            "import 'package:flutter/foundation.dart'; \
             void f() {{ if (kDebugMode) {{ print({chain}); \
             void nested() {{ print('nested'); }} nested(); \
             }} }}"
        );
        let stdout = check_rule_source(
            "suspicious",
            "avoid-print",
            &source,
            &[
                ("pubspec.yaml", "name: chain_test\n"),
                (
                    ".dart_tool/package_config.json",
                    r#"{"configVersion":2,"packages":[{"name":"flutter","rootUri":"../flutter","packageUri":"lib/"}]}"#,
                ),
                (
                    "flutter/lib/foundation.dart",
                    "export 'src/foundation/constants.dart';\n",
                ),
                (
                    "flutter/lib/src/foundation/constants.dart",
                    "const bool kDebugMode = true;\n",
                ),
                (
                    "sdk/lib/libraries.json",
                    r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
                ),
                (
                    "sdk/lib/core/core.dart",
                    include_str!("../crates/falcon_rules/tests/support/sdk/lib/core/core.dart"),
                ),
            ],
        );
        let diagnostics: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let printed = diagnostics
            .as_array()
            .unwrap()
            .iter()
            .filter(|diagnostic| diagnostic["rule"] == "avoid-print")
            .count();
        assert_eq!(
            printed, 1,
            "guarded print must be exempt, nested print must not"
        );
    }

    #[test]
    fn library_target_annotation() {
        const CHILD: &str = "FALCON_LIBRARY_TARGET_CHAIN_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "guard_walkers::library_target_annotation",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env_remove("RUST_MIN_STACK")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\nstdout: {}\nstderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }
        use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
        use falcon_config::FalconConfig;
        use falcon_rules::lint::style::library_annotations::LibraryAnnotations;
        use std::path::PathBuf;
        use std::sync::Arc;

        let chain = format!("1{}", " + 1".repeat(99_999));
        let source = format!(
            "@A() import 'dart:core'; \
             @Target([{chain}, TargetKind.library]) class A {{ const A(); }}"
        );
        let parsed = falcon_dart_parser::parse(&source);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let path = PathBuf::from("main.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        let diagnostics = LibraryAnnotations.analyze(
            "library-annotations",
            parsed.tree.program(&parsed.source),
            &ctx,
        );
        assert_eq!(diagnostics.len(), 1);
    }
}

mod body_walkers {
    use super::semantic_walkers::check_rule_source;

    #[test]
    fn generator_yields() {
        let chain = format!("1{}", " + 1".repeat(99_999));
        let source = format!(
            "void f<T>(T? x) {{ final generator = () sync* {{ {chain}; yield x; yield x!; }}; }}"
        );
        let stdout = check_rule_source(
            "correctness",
            "null-check-on-nullable-type-parameter",
            &source,
            &[],
        );
        assert!(stdout.contains("null-check-on-nullable-type-parameter"));
    }

    #[test]
    fn sibling_part_writes() {
        let chain = format!("1{}", " + 1".repeat(99_999));
        let part = format!("part of fields; void f() {{ {chain}; }}");
        let stdout = check_rule_source(
            "style",
            "prefer-final-fields",
            "library fields; part 'part.dart'; class C { int _value = 0; }",
            &[("part.dart", &part)],
        );
        assert!(stdout.contains("prefer-final-fields"));
    }

    #[test]
    fn dispose_body_scopes() {
        let chain = format!("true{}", " && true".repeat(99_999));
        let source = format!(
            "import 'package:flutter/material.dart'; \
             class C extends State<Widget> {{ \
             final controller = TextEditingController(); \
             final unreleased = TextEditingController(); \
             void dispose() {{ {chain}; controller.dispose(); super.dispose(); }} }}"
        );
        let stdout = check_rule_source(
            "correctness",
            "proper-controller-dispose",
            &source,
            &[
                (
                    "pubspec.yaml",
                    "name: chain_test\ndependencies:\n  flutter:\n    path: flutter\n",
                ),
                (
                    ".dart_tool/package_config.json",
                    r#"{"configVersion":2,"packages":[{"name":"flutter","rootUri":"../flutter","packageUri":"lib/"}]}"#,
                ),
                (
                    "flutter/lib/material.dart",
                    "class Widget {}\nabstract class State<T extends Widget> { void dispose() {} }\nclass TextEditingController { void dispose() {} }\n",
                ),
            ],
        );
        let diagnostics: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let undisposed = diagnostics
            .as_array()
            .unwrap()
            .iter()
            .filter(|diagnostic| diagnostic["rule"] == "proper-controller-dispose")
            .count();
        assert_eq!(
            undisposed, 1,
            "only the unreleased controller should be reported"
        );
    }
}
