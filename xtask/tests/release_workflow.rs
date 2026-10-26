#![cfg(unix)]

use std::{fs, process::Command};

const CREDENTIALS: [(&str, &str); 9] = [
    ("HAS_APPLE_CERTIFICATE", "secrets.APPLE_CERTIFICATE_P12"),
    (
        "HAS_APPLE_CERTIFICATE_PASSWORD",
        "secrets.APPLE_CERTIFICATE_PASSWORD",
    ),
    ("HAS_APPLE_ID", "secrets.APPLE_ID"),
    ("HAS_APPLE_TEAM_ID", "secrets.APPLE_TEAM_ID"),
    ("HAS_APPLE_APP_PASSWORD", "secrets.APPLE_APP_PASSWORD"),
    ("HAS_MINISIGN_KEY", "secrets.MINISIGN_SECRET_KEY"),
    ("HAS_MINISIGN_PASSWORD", "secrets.MINISIGN_PASSWORD"),
    ("HAS_TAP_TOKEN", "secrets.HOMEBREW_TAP_TOKEN"),
    ("HAS_TAP", "vars.HOMEBREW_TAP"),
];

fn check_credentials(configured: [bool; 9], expected_success: bool) {
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
    assert!(step.get("if").is_none(), "credential validation must always run");
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("output");
    fs::write(&output_path, "").unwrap();
    let mut command = Command::new("bash");
    command
        .args(["-c", step["run"].as_str().unwrap()])
        .env("GITHUB_OUTPUT", &output_path);
    for ((name, expression), present) in CREDENTIALS.iter().zip(configured) {
        assert_eq!(
            step["env"][*name].as_str().unwrap(),
            format!("${{{{ {expression} != '' }}}}"),
        );
        command.env(name, if present { "true" } else { "false" });
    }
    let output = command.output().unwrap();
    assert_eq!(
        output.status.success(),
        expected_success,
        "credentials {configured:?}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        if expected_success {
            format!("homebrew_enabled={}\n", configured[7])
        } else {
            String::new()
        },
    );
}

#[test]
fn release_rejects_every_partial_apple_signing_configuration() {
    for mask in 0..32 {
        let mut configured = [false; 9];
        for (bit, present) in configured[..5].iter_mut().enumerate() {
            *present = mask & (1 << bit) != 0;
        }
        check_credentials(configured, mask == 0 || mask == 31);
    }
}

#[test]
fn release_requires_both_minisign_secrets_or_neither() {
    for mask in 0..4 {
        let mut configured = [false; 9];
        configured[5] = mask & 1 != 0;
        configured[6] = mask & 2 != 0;
        check_credentials(configured, mask == 0 || mask == 3);
    }
}

#[test]
fn release_requires_a_token_for_a_custom_homebrew_tap() {
    for mask in 0..4 {
        let mut configured = [false; 9];
        configured[7] = mask & 1 != 0;
        configured[8] = mask & 2 != 0;
        // A token alone uses the workflow's default tap.
        check_credentials(configured, mask != 2);
    }
}

#[test]
fn release_accepts_all_optional_publishing_credentials() {
    check_credentials([true; 9], true);
}
