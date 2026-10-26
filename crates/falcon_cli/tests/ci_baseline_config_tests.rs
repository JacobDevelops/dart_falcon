use falcon_cli::{CheckOptions, collect_check_with_sdk_locator};
use std::fs;

#[test]
fn configured_baseline_uses_project_root_and_cli_baseline_takes_precedence() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("falcon.json");
    fs::write(
        project.path().join("main.dart"),
        "void f(dynamic value) {}\n",
    )
    .unwrap();
    let settings = serde_json::json!({
        "baseline": "debt.json",
        "linter": {"rules": {"recommended": false, "suspicious": {"avoid-dynamic": "error"}}},
        "cross-file": {"rules": {"recommended": false}}
    });
    fs::write(&config, settings.to_string()).unwrap();
    let baseline = project.path().join("debt.json");
    let mut options = CheckOptions {
        paths: vec![project.path().to_path_buf()],
        config_path: Some(config.clone()),
        write_baseline: Some(baseline),
        ..Default::default()
    };
    collect_check_with_sdk_locator(&options, |_| None).unwrap();
    options.write_baseline = None;
    let output = collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert!(output.diagnostics.is_empty());
    assert_eq!(output.exit_code, 0);
    let empty = project.path().join("empty.json");
    fs::write(&empty, r#"{"version":1,"entries":{}}"#).unwrap();
    options.baseline = Some(empty);
    let output = collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.exit_code, 1);
}

#[test]
fn writing_baseline_does_not_destroy_existing_debt_after_operational_failure() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("falcon.json");
    fs::write(&config, "{}").unwrap();
    let baseline = project.path().join("debt.json");
    let original = r#"{"version":1,"entries":{}}"#;
    fs::write(&baseline, original).unwrap();
    let options = CheckOptions {
        paths: vec![project.path().join("missing.dart")],
        config_path: Some(config),
        write_baseline: Some(baseline.clone()),
        ..Default::default()
    };
    let error = collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
    assert!(error.contains("Cannot write a baseline"));
    assert_eq!(fs::read_to_string(baseline).unwrap(), original);
}
