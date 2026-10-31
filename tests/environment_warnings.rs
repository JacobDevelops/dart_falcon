use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn discovered_broken_manifests_warn_without_enclosing_analyzed_sources() {
    for nested_sources in [false, true] {
        for broken in [b"homepage: [".as_slice(), &[0xff], b"oversized"] {
            let project = tempdir().unwrap();
            let manifest = project.path().join("pubspec.yaml");
            fs::write(&manifest, broken).unwrap();
            if broken == b"oversized" {
                fs::File::options()
                    .write(true)
                    .open(&manifest)
                    .unwrap()
                    .set_len(16 * 1024 * 1024 + 1)
                    .unwrap();
            }
            fs::write(project.path().join("falcon.json"), r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}}"#).unwrap();
            if nested_sources {
                let package = project.path().join("packages/a");
                fs::create_dir_all(&package).unwrap();
                fs::write(package.join("pubspec.yaml"), "name: a\n").unwrap();
                fs::write(package.join("main.dart"), "void main() {}\n").unwrap();
            }
            let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
                .args(["check", ".", "--format", "json"])
                .current_dir(project.path())
                .env_remove("DART_SDK")
                .env_remove("FLUTTER_ROOT")
                .env("PATH", "")
                .output()
                .unwrap();
            if nested_sources || broken == b"homepage: [" {
                assert!(output.status.success(), "{output:?}");
            } else {
                assert_eq!(output.status.code(), Some(1));
                assert!(String::from_utf8_lossy(&output.stdout).contains("unmatched-path"));
            }
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert_eq!(
                stderr
                    .lines()
                    .filter(|line| line.contains("warning:")
                        && line.contains(&*manifest.to_string_lossy()))
                    .count(),
                1,
                "{stderr}"
            );
        }
    }
}

#[test]
fn optional_manifest_warning_and_secure_url_recovery_reach_cli_output() {
    for broken in [b"homepage: [".as_slice(), &[0xff], b"oversized"] {
        let project = tempdir().unwrap();
        let manifest = project.path().join("pubspec.yaml");
        fs::write(project.path().join("main.dart"), "void main() {}\n").unwrap();
        fs::write(project.path().join("falcon.json"), r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"security":{"secure-pubspec-urls":"warn"}}}}"#).unwrap();
        fs::write(&manifest, broken).unwrap();
        if broken == b"oversized" {
            fs::File::options()
                .write(true)
                .open(&manifest)
                .unwrap()
                .set_len(16 * 1024 * 1024 + 1)
                .unwrap();
        }
        let check = || {
            Command::new(env!("CARGO_BIN_EXE_falcon"))
                .args(["check", ".", "--format", "json"])
                .current_dir(project.path())
                .env_remove("DART_SDK")
                .env_remove("FLUTTER_ROOT")
                .env("PATH", "")
                .output()
                .unwrap()
        };
        let failed = check();
        assert!(failed.status.success(), "{failed:?}");
        let stderr = String::from_utf8(failed.stderr).unwrap();
        assert_eq!(
            stderr
                .lines()
                .filter(|line| line.contains("warning:") && line.contains("pubspec.yaml"))
                .count(),
            1,
            "{stderr}"
        );
        assert!(
            !String::from_utf8(failed.stdout)
                .unwrap()
                .contains("secure-pubspec-urls")
        );
        fs::write(&manifest, "name: example\nhomepage: http://example.com\n").unwrap();
        let recovered = check();
        let stdout = String::from_utf8(recovered.stdout).unwrap();
        assert!(stdout.contains("secure-pubspec-urls"), "{stdout}");
        assert!(
            stdout.contains("The `http` protocol shouldn't be used because it isn't secure."),
            "{stdout}"
        );
        assert!(
            !String::from_utf8(recovered.stderr)
                .unwrap()
                .contains("warning:")
        );
    }
}

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
        assert!(output.status.success(), "{output:?}");
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
#[test]
fn companion_scope_is_independent_of_cli_current_directory_and_spelling() {
    let directory = tempdir().unwrap();
    let project = directory.path().join("app");
    let src = project.join("lib/src");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(project.join("lib/models")).unwrap();
    fs::create_dir_all(project.join(".dart_tool")).unwrap();
    fs::write(project.join("pubspec.yaml"), "name: app\n").unwrap();
    let config = directory.path().join("falcon.json");
    fs::write(&config, r#"{"linter":{"rules":{"recommended":false,"correctness":{"use-key-in-widget-constructors":"warn"}}}}"#).unwrap();
    let cache = directory.path().join("pub-cache");
    fs::create_dir_all(cache.join("lib")).unwrap();
    fs::write(cache.join("pubspec.yaml"), "name: cached\n").unwrap();
    fs::write(cache.join("lib/outside.dart"), [0xff]).unwrap();
    fs::write(project.join(".dart_tool/package_config.json"), r#"{"configVersion":2,"packages":[{"name":"cached","rootUri":"../../pub-cache/","packageUri":"lib/"}]}"#).unwrap();
    fs::write(project.join("lib/models/companion.dart"), [0xff]).unwrap();
    let main = src.join("main.dart");
    for required in [true, false] {
        fs::write(
            &main,
            if required {
                "import '../models/companion.dart'; void main() {}"
            } else {
                "import 'package:cached/outside.dart'; void main() {}"
            },
        )
        .unwrap();
        for (cwd, relative) in [
            (project.as_path(), "lib/src/main.dart"),
            (directory.path(), "app/lib/src/main.dart"),
            (src.as_path(), "main.dart"),
        ] {
            for spelling in [
                relative.to_owned(),
                format!("./{relative}"),
                main.display().to_string(),
            ] {
                let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
                    .current_dir(cwd)
                    .args(["check", &spelling, "--config"])
                    .arg(&config)
                    .env_remove("DART_SDK")
                    .env_remove("FLUTTER_ROOT")
                    .env("PATH", "")
                    .output()
                    .unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert_eq!(
                    output.status.success(),
                    !required,
                    "{cwd:?} {spelling}: {stderr}"
                );
                assert!(
                    stderr.contains(if required {
                        "companion.dart"
                    } else {
                        "outside.dart"
                    }),
                    "{stderr}"
                );
                assert!(stderr.contains("UTF-8"), "{stderr}");
                if !required {
                    assert!(stderr.contains("warning:"), "{stderr}");
                }
            }
        }
    }
}

#[test]
fn loose_file_write_rejects_unreadable_parts_without_changing_source() {
    let directory = tempdir().unwrap();
    let main = directory.path().join("main.dart");
    let source = "part 'broken.dart'; class Example {} void main() { new Example(); }";
    fs::write(&main, source).unwrap();
    fs::write(directory.path().join("broken.dart"), [0xff]).unwrap();
    fs::write(
        directory.path().join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false,"style":{"unnecessary-new":"warn"}}}}"#,
    )
    .unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_falcon"))
            .current_dir(directory.path())
            .args(["check", "--write", "./main.dart"])
            .env_remove("DART_SDK")
            .env_remove("FLUTTER_ROOT")
            .env("PATH", "")
            .output()
            .unwrap()
    };
    let failed = run();
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(!failed.status.success(), "{stderr}");
    assert!(stderr.contains("broken.dart"), "{stderr}");
    assert!(stderr.contains("UTF-8"), "{stderr}");
    assert_eq!(fs::read_to_string(&main).unwrap(), source);
    fs::write(directory.path().join("broken.dart"), "part of 'main.dart';").unwrap();
    let recovered = run();
    assert!(recovered.status.success(), "{recovered:?}");
    assert!(!fs::read_to_string(&main).unwrap().contains("new Example"));
}

#[test]
fn collected_package_companions_abort_cli_analysis() {
    let directory = tempdir().unwrap();
    for package in ["affected", "observer"] {
        fs::create_dir_all(directory.path().join(package).join("lib")).unwrap();
        fs::write(
            directory.path().join(package).join("pubspec.yaml"),
            format!("name: {package}\n"),
        )
        .unwrap();
    }
    fs::write(
        directory.path().join("affected/lib/main.dart"),
        "import 'companion.dart'; void main() {}",
    )
    .unwrap();
    fs::write(
        directory.path().join("observer/lib/main.dart"),
        "void main() {}",
    )
    .unwrap();
    fs::write(
        directory.path().join("affected/lib/orphan.dart"),
        "class Orphan {}",
    )
    .unwrap();
    let companion = directory.path().join("affected/lib/companion.dart");
    fs::write(&companion, [0xff]).unwrap();
    fs::write(directory.path().join("falcon.json"), r#"{"files":{"includes":["**","!affected/lib/companion.dart"]},"linter":{"rules":{"recommended":false,"correctness":{"use-key-in-widget-constructors":"warn"}}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .current_dir(directory.path())
        .args(["check", "."])
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env("PATH", "")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("companion.dart"), "{stderr}");
    assert!(stderr.contains("UTF-8"), "{stderr}");
}
