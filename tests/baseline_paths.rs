use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn check(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
        .arg("check")
        .args(arguments)
        .current_dir(directory)
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env("HOME", directory)
        .env("PATH", "")
        .output()
        .unwrap()
}

#[test]
fn baseline_written_at_root_matches_from_subdirectory() {
    for workspace in [false, true] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        let member = root.join("app");
        fs::create_dir(&member).unwrap();
        if workspace {
            fs::write(root.join("pubspec.yaml"), "name: repo\nworkspace:\n  - app\n").unwrap();
            fs::write(member.join("pubspec.yaml"), "name: app\nresolution: workspace\n").unwrap();
        } else {
            fs::create_dir(root.join(".git")).unwrap();
        }
        fs::write(member.join("main.dart"), "void main() { print('hello'); }\n").unwrap();
        let written = check(root, &[".", "--write-baseline", "debt.json", "--format", "json"]);
        assert!(root.join("debt.json").is_file(), "{}", String::from_utf8_lossy(&written.stderr));
        let output = check(&member, &[".", "--baseline", "../debt.json", "--format", "json"]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("0 fixed (stale)"), "{stderr}");
        assert!(!stderr.contains("Baseline: 0 suppressed"), "{stderr}");
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let diagnostics = json.get("diagnostics").unwrap_or(&json).as_array().unwrap();
        assert!(diagnostics.is_empty(), "{json}");
    }
}

#[test]
fn bare_baseline_write_uses_configured_path_or_anchored_root() {
    for configured in [false, true] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        let member = root.join("app");
        fs::create_dir(&member).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(member.join("main.dart"), "void main() { print('hello'); }\n").unwrap();
        if configured {
            fs::write(root.join("falcon.json"), r#"{"baseline":"debt.json"}"#).unwrap();
        }
        let output = check(&member, &[".", "--write-baseline", "--format", "json"]);
        let name = if configured { "debt.json" } else { "falcon-baseline.json" };
        assert!(root.join(name).is_file(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(!member.join(name).exists());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Cannot read baseline"));
    }
}

#[test]
fn explicit_baseline_write_stays_relative_to_current_directory() {
    let project = tempfile::tempdir().unwrap();
    let member = project.path().join("app");
    fs::create_dir(&member).unwrap();
    fs::create_dir(project.path().join(".git")).unwrap();
    fs::write(member.join("main.dart"), "void main() { print('hello'); }\n").unwrap();
    let output = check(&member, &[".", "--write-baseline", "local.json"]);
    assert!(member.join("local.json").is_file(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!project.path().join("local.json").exists());
}
