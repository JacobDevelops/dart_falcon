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
