use falcon_cli::{CheckOptions, collect_check_with_sdk_locator};
use std::fs;
use tempfile::tempdir;

fn collect(options: &CheckOptions) -> falcon_cli::CheckOutput {
    collect_check_with_sdk_locator(options, |_| None).unwrap()
}

#[test]
fn severity_threshold_respects_cli_config_and_custom_exit_code() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("falcon.json");
    fs::write(dir.path().join("main.dart"), "void f(dynamic value) {}\n").unwrap();
    for (level, configured, flag, expected) in [
        ("warn", false, false, 0),
        ("warn", false, true, 7),
        ("warn", true, false, 7),
        ("info", true, true, 0),
        ("error", false, false, 7),
    ] {
        fs::write(
            &config,
            serde_json::json!({
                "error-on-warnings": configured,
                "linter": {"rules": {"recommended": false, "suspicious": {"avoid-dynamic": level}}},
                "cross-file": {"rules": {"recommended": false}}
            })
            .to_string(),
        )
        .unwrap();
        let output = collect(&CheckOptions {
            paths: vec![dir.path().to_path_buf()],
            config_path: Some(config.clone()),
            error_on_warnings: flag,
            error_exit_code: 7,
            ..Default::default()
        });
        assert_eq!(output.diagnostics.len(), 1, "{level}");
        assert_eq!(
            output.exit_code, expected,
            "{level} configured={configured} flag={flag}"
        );
    }
}

#[test]
fn baseline_tracks_content_across_line_shifts_and_counts_duplicates() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("falcon.json");
    let source = dir.path().join("main.dart");
    let baseline = dir.path().join("debt.json");
    fs::write(&config, r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"rules":{"recommended":false}}}"#).unwrap();
    fs::write(&source, "void f(dynamic value) {}\n").unwrap();
    let mut options = CheckOptions {
        paths: vec![dir.path().to_path_buf()],
        config_path: Some(config),
        write_baseline: Some(Some(baseline.clone())),
        ..Default::default()
    };
    collect(&options);
    assert!(baseline.is_file());
    options.write_baseline = None;
    options.baseline = Some(baseline);
    fs::write(&source, "// inserted\n\nvoid f(dynamic value) {}\n").unwrap();
    let shifted = collect(&options);
    assert!(shifted.diagnostics.is_empty());
    assert_eq!(shifted.exit_code, 0);
    fs::write(
        &source,
        "// inserted\n\nvoid f(dynamic value) {}\nvoid g(dynamic value) {}\n",
    )
    .unwrap();
    let duplicate = collect(&options);
    assert_eq!(duplicate.diagnostics.len(), 1);
    assert_eq!(duplicate.exit_code, 1);
}
