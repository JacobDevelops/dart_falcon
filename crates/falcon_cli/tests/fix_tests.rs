use falcon_cli::{CheckOptions, CheckOutput, collect_check_with_sdk_locator};
use serde_json::json;
use std::fs;
use tempfile::{TempDir, tempdir};

const BEFORE: &str = "class Example {}\nvoid main() { final value = new Example(); }\n";
const AFTER: &str = "class Example {}\nvoid main() { final value = Example(); }\n";

fn fixture(rule: &serde_json::Value) -> (TempDir, CheckOptions) {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("falcon.json");
    fs::write(
        &config_path,
        json!({
            "linter": { "rules": { "recommended": false, "style": {
                "unnecessary-new": rule
            }}},
            "cross-file": { "enabled": false }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(dir.path().join("main.dart"), BEFORE).unwrap();
    let options = CheckOptions {
        paths: vec![dir.path().to_path_buf()],
        config_path: Some(config_path),
        write: true,
        quiet: true,
        ..Default::default()
    };
    (dir, options)
}

fn collect(options: &CheckOptions) -> CheckOutput {
    collect_check_with_sdk_locator(options, |_| None).unwrap()
}

#[test]
fn write_applies_safe_fixes_and_reports_final_source() {
    let (dir, options) = fixture(&json!("error"));
    let output = collect(&options);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        AFTER
    );
    assert!(output.diagnostics.is_empty());
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.total_files, 1);
    assert_eq!(output.diff, "");
    assert_eq!(
        falcon_cli::run_check_with_sdk_locator(&options, |_| None),
        0
    );
}

#[test]
fn check_without_write_never_changes_source() {
    let (dir, mut options) = fixture(&json!("error"));
    options.write = false;
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
}

#[test]
fn unsafe_reclassification_requires_explicit_opt_in() {
    let (dir, mut options) = fixture(&json!({ "level": "error", "fix": "unsafe" }));
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
    options.unsafe_fixes = true;
    assert_eq!(collect(&options).exit_code, 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        AFTER
    );
}

#[test]
fn none_classification_prevents_even_unsafe_writes() {
    let (dir, mut options) = fixture(&json!({ "level": "error", "fix": "none" }));
    options.unsafe_fixes = true;
    assert_eq!(collect(&options).exit_code, 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
}

#[test]
fn path_override_controls_fix_classification() {
    let (dir, mut options) = fixture(&json!("error"));
    let config_path = options.config_path.as_ref().unwrap();
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_path).unwrap()).unwrap();
    config["overrides"] = json!([{
        "includes": ["**/main.dart"],
        "linter": { "rules": { "style": {
            "unnecessary-new": { "level": "error", "fix": "none" }
        }}}
    }]);
    fs::write(config_path, config.to_string()).unwrap();
    fs::write(dir.path().join("other.dart"), BEFORE).unwrap();
    options.unsafe_fixes = true;
    let output = collect(&options);
    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].file_path.ends_with("main.dart"));
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("other.dart")).unwrap(),
        AFTER
    );
}

#[test]
fn dry_run_previews_diff_and_reports_original_diagnostics_without_writes() {
    let (dir, mut options) = fixture(&json!("error"));
    options.dry_run = true;
    let output = collect(&options);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics[0].span.start,
        BEFORE.find("new ").unwrap()
    );
    assert!(
        output
            .diff
            .starts_with("--- a/main.dart\n+++ b/main.dart\n")
    );
    assert!(output.diff.contains("@@ -1,2 +1,2 @@\n"));
    assert!(
        output
            .diff
            .contains("-void main() { final value = new Example(); }\n")
    );
    assert!(
        output
            .diff
            .contains("+void main() { final value = Example(); }\n")
    );
}

#[test]
fn dry_run_marks_missing_final_newlines() {
    let (dir, mut options) = fixture(&json!("error"));
    options.dry_run = true;
    let source = BEFORE.trim_end_matches('\n');
    fs::write(dir.path().join("main.dart"), source).unwrap();
    let output = collect(&options);
    assert_eq!(
        output
            .diff
            .matches("\\ No newline at end of file\n")
            .count(),
        2
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        source
    );
}

#[test]
fn fixing_preserves_final_cross_file_diagnostics() {
    let (dir, options) = fixture(&json!("error"));
    let config_path = options.config_path.as_ref().unwrap();
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_path).unwrap()).unwrap();
    config["cross-file"] = json!({ "rules": { "recommended": false, "security": {
        "secure-pubspec-urls": "error"
    }}});
    fs::write(config_path, config.to_string()).unwrap();
    fs::write(
        dir.path().join("pubspec.yaml"),
        "name: example\nhomepage: http://example.com\n",
    )
    .unwrap();
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].rule, "secure-pubspec-urls");
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        AFTER
    );
}

#[test]
fn cycles_report_stops_and_still_write_converged_files() {
    let (dir, options) = fixture(&json!("error"));
    let config_path = options.config_path.as_ref().unwrap();
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_path).unwrap()).unwrap();
    config["linter"]["rules"]["style"]["prefer-single-quotes"] = json!("error");
    config["linter"]["rules"]["style"]["prefer-double-quotes"] = json!("error");
    fs::write(config_path, config.to_string()).unwrap();
    let source = "void main() { final value = 'hello'; }\n";
    fs::write(dir.path().join("main.dart"), source).unwrap();
    let earlier = dir.path().join("aaa.dart");
    fs::write(&earlier, BEFORE).unwrap();
    let output = collect(&options);
    let stop = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule == "autofix")
        .unwrap();
    assert!(stop.message.contains("prefer-single-quotes"), "{stop:?}");
    assert!(stop.message.contains("prefer-double-quotes"), "{stop:?}");
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        source
    );
    assert_eq!(fs::read_to_string(earlier).unwrap(), AFTER);
}

#[test]
fn direct_pipeline_call_rejects_fix_modifiers_without_write() {
    for (unsafe_fixes, dry_run) in [(true, false), (false, true)] {
        let options = CheckOptions {
            unsafe_fixes,
            dry_run,
            ..Default::default()
        };
        let error = collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
        assert!(error.contains("require --write"));
    }
}

#[test]
fn dry_run_rejects_baseline_writes_without_changing_files() {
    let (dir, mut options) = fixture(&json!("error"));
    let baseline = dir.path().join("baseline.json");
    fs::write(&baseline, "existing baseline").unwrap();
    options.dry_run = true;
    options.write_baseline = Some(Some(baseline.clone()));
    let error = collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
    assert!(error.contains("--dry-run"), "{error}");
    assert!(error.contains("--write-baseline"), "{error}");
    assert_eq!(fs::read_to_string(baseline).unwrap(), "existing baseline");
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        BEFORE
    );
}

#[test]
fn large_converging_files_are_written() {
    let (dir, options) = fixture(&json!("error"));
    let source = format!(
        "class Example {{}}\nvoid main() {{\n{}}}\n",
        "new Example();\n".repeat(101)
    );
    let path = dir.path().join("main.dart");
    fs::write(&path, &source).unwrap();
    let output = collect(&options);
    assert_eq!(output.exit_code, 0);
    assert!(output.diagnostics.is_empty());
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        source.replace("new ", "")
    );
}

#[test]
fn dry_run_pending_warning_fixes_fail_but_clean_dry_runs_succeed() {
    let (dir, mut options) = fixture(&json!("warn"));
    options.dry_run = true;
    options.error_exit_code = 0;
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert_ne!(output.diff, "");
    fs::write(dir.path().join("main.dart"), AFTER).unwrap();
    let output = collect(&options);
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.diff, "");
}

#[test]
fn syntax_errors_are_reported_and_other_files_are_fixed() {
    let (dir, options) = fixture(&json!("error"));
    let source = format!("{BEFORE}void broken( {{\n");
    fs::write(dir.path().join("main.dart"), &source).unwrap();
    fs::write(dir.path().join("other.dart"), BEFORE).unwrap();
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "syntax-error")
    );
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic.rule == "autofix" && diagnostic.message.contains("syntax")));
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("other.dart")).unwrap(),
        AFTER
    );
}

#[cfg(unix)]
#[test]
fn symlinked_files_are_reported_and_never_rewritten() {
    let (dir, options) = fixture(&json!("error"));
    let outside = tempdir().unwrap();
    let target = outside.path().join("target.dart");
    fs::write(&target, BEFORE).unwrap();
    let link = dir.path().join("linked.dart");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let output = collect(&options);
    assert_eq!(output.exit_code, 1);
    assert!(
        output.diagnostics.iter().any(
            |diagnostic| diagnostic.rule == "autofix" && diagnostic.message.contains("symlink")
        )
    );
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), BEFORE);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        AFTER
    );
}

#[cfg(unix)]
#[test]
fn atomic_writes_replace_inodes_and_preserve_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let (dir, options) = fixture(&json!("error"));
    let path = dir.path().join("main.dart");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let original = fs::File::open(&path).unwrap();
    let inode = original.metadata().unwrap().ino();
    assert_eq!(collect(&options).exit_code, 0);
    let metadata = fs::metadata(&path).unwrap();
    assert_ne!(metadata.ino(), inode);
    assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
    assert_eq!(fs::read_to_string(path).unwrap(), AFTER);
}

#[test]
fn read_only_sources_are_reported_and_preserve_original_diagnostics() {
    let (dir, options) = fixture(&json!("error"));
    let path = dir.path().join("main.dart");
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let output = collect(&options);
    fs::set_permissions(&path, original_permissions).unwrap();
    assert_eq!(output.exit_code, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule == "autofix" && diagnostic.message.contains("read-only")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule == "unnecessary-new"
            && diagnostic.span.start == BEFORE.find("new ").unwrap()
    }));
    assert_eq!(fs::read_to_string(path).unwrap(), BEFORE);
}

#[test]
fn autofix_reports_are_sorted_with_other_diagnostics() {
    let (dir, options) = fixture(&json!("error"));
    let first = dir.path().join("aaa.dart");
    fs::write(&first, BEFORE).unwrap();
    let original_permissions = fs::metadata(&first).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&first, permissions).unwrap();
    let output = collect(&options);
    fs::set_permissions(&first, original_permissions).unwrap();
    assert_eq!(output.diagnostics[0].rule, "autofix");
    assert!(output.diagnostics[0].file_path.ends_with("aaa.dart"));
    assert_eq!(
        fs::read_to_string(dir.path().join("main.dart")).unwrap(),
        AFTER
    );
}
