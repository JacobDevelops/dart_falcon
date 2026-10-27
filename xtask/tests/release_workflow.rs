#![cfg(unix)]

use std::{fs, process::Command};

const CREDENTIALS: [(&str, &str); 7] = [
    ("HAS_APPLE_CERTIFICATE", "secrets.APPLE_CERTIFICATE_P12"),
    (
        "HAS_APPLE_CERTIFICATE_PASSWORD",
        "secrets.APPLE_CERTIFICATE_PASSWORD",
    ),
    ("HAS_APPLE_ID", "secrets.APPLE_ID"),
    ("HAS_APPLE_TEAM_ID", "secrets.APPLE_TEAM_ID"),
    ("HAS_APPLE_APP_PASSWORD", "secrets.APPLE_APP_PASSWORD"),
    ("HAS_MINISIGN_KEY", "secrets.MINISIGN_SECRET_KEY"),
    ("HAS_TAP_TOKEN", "secrets.HOMEBREW_TAP_TOKEN"),
];

fn check_credentials(configured: [bool; 7]) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let workflow: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(root.join(".github/workflows/release.yml")).unwrap(),
    )
    .unwrap();
    let step = workflow["jobs"]["resolve"]["steps"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|step| step["id"].as_str() == Some("packaging"))
        .unwrap();
    assert!(
        step.get("if").is_none(),
        "credential validation must always run"
    );
    let mut command = Command::new("bash");
    command.args(["-c", step["run"].as_str().unwrap()]);
    for ((name, expression), present) in CREDENTIALS.iter().zip(configured) {
        assert_eq!(
            step["env"][*name].as_str().unwrap(),
            format!("${{{{ {expression} != '' }}}}"),
        );
        command.env(name, if present { "true" } else { "false" });
    }
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.success(),
        configured.iter().all(|present| *present),
        "credentials {configured:?}: {stderr}",
    );
    for ((_, expression), present) in CREDENTIALS.iter().zip(configured) {
        let secret = expression.trim_start_matches("secrets.");
        assert_eq!(stderr.contains(secret), !present, "{secret}: {stderr}");
    }
}

#[test]
fn release_requires_every_signing_and_publishing_secret() {
    for mask in 0..1 << CREDENTIALS.len() {
        check_credentials(std::array::from_fn(|bit| mask & (1 << bit) != 0));
    }
}
