use std::process::Command;

#[test]
fn malformed_package_config_is_reported_on_stderr() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir(project.path().join(".dart_tool")).unwrap();
    let package_config = project.path().join(".dart_tool/package_config.json");
    std::fs::write(&package_config, "{").unwrap();
    std::fs::write(project.path().join("main.dart"), "void main() {}\n").unwrap();
    std::fs::write(project.path().join("falcon.json"), "{}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args(["check", ".", "--quiet", "--exit-code", "0"])
        .current_dir(project.path())
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("package_config.json"), "{stderr}");
    assert!(stderr.contains("warning:"), "{stderr}");
}

#[test]
fn generated_empty_uri_self_import_has_no_warning_on_stderr() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("main.dart"),
        "import '' as self;\nclass Generated {}\nself.Generated? binding;\n",
    )
    .unwrap();
    std::fs::write(project.path().join("falcon.json"), "{}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args(["check", ".", "--quiet", "--exit-code", "0"])
        .current_dir(project.path())
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("warning:"), "{stderr}");
}

#[test]
fn member_run_wide_settings_warn_once_on_stderr_only_when_present() {
    for (settings, expected_keys) in [
        (
            r#""max-errors":null,"error-on-warnings":false,"baseline":null"#,
            &["max-errors", "error-on-warnings", "baseline"][..],
        ),
        (r#""max_errors":null"#, &["max_errors"][..]),
        ("", &[][..]),
    ] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        std::fs::write(
            root.join("pubspec.yaml"),
            "name: workspace_root\nworkspace:\n  - packages/a\n  - packages/b\n",
        )
        .unwrap();
        std::fs::write(root.join("falcon.json"), r#"{"linter":{"enabled":false}}"#).unwrap();
        for name in ["a", "b"] {
            let member = root.join("packages").join(name);
            std::fs::create_dir_all(member.join("lib")).unwrap();
            std::fs::write(
                member.join("pubspec.yaml"),
                format!("name: {name}\nresolution: workspace\n"),
            )
            .unwrap();
            for file in ["first.dart", "second.dart"] {
                std::fs::write(member.join("lib").join(file), "void main() {}\n").unwrap();
            }
        }
        let member_config = root.join("packages/falcon.json");
        let separator = if settings.is_empty() { "" } else { "," };
        std::fs::write(
            &member_config,
            format!(r#"{{"linter":{{"enabled":false}}{separator}{settings}}}"#),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
            .args([
                "check",
                ".",
                "--quiet",
                "--exit-code",
                "0",
                "--format",
                "json",
            ])
            .current_dir(root)
            .env_remove("DART_SDK")
            .env_remove("FLUTTER_ROOT")
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            stderr.matches("ignores run-wide setting").count(),
            expected_keys.len(),
            "{stderr}"
        );
        for key in expected_keys {
            let warning = format!(
                "warning: {}: member configuration ignores run-wide setting `{key}`",
                member_config.display()
            );
            assert_eq!(stderr.matches(&warning).count(), 1, "{stderr}");
        }
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(!stdout.contains("ignores run-wide setting"), "{stdout}");
    }
}
