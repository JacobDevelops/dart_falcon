#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn executable(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn perf_lock_runs_the_built_binary_in_the_cargo_target_directory() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("custom target");
    fs::create_dir_all(target.join("release")).unwrap();
    let marker = directory.path().join("invocations");
    executable(
        &target.join("release/falcon"),
        "#!/bin/sh\nprintf 'ran\\n' >> \"$PERF_MARKER\"\n",
    );
    let cargo = directory.path().join("cargo");
    executable(
        &cargo,
        "#!/bin/sh\ncase \"$1\" in\n  build) exit 0 ;;\n  metadata) printf '{\"target_directory\":\"%s\"}' \"$CARGO_TARGET_DIR\" ;;\n  *) exit 1 ;;\nesac\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["perf-lock", "--runs", "1", "--budget-ms", "60000"])
        .arg("--corpus")
        .arg(directory.path())
        .env("CARGO_CMD", cargo)
        .env("CARGO_TARGET_DIR", target)
        .env("PERF_MARKER", &marker)
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(marker).unwrap(), "ran\nran\n");
}

#[test]
fn perf_lock_runs_the_explicit_binary_without_cargo() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("falcon");
    let marker = directory.path().join("invocations");
    executable(&binary, "#!/bin/sh\nprintf 'ran\\n' >> \"$PERF_MARKER\"\n");
    for skip_build in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command.args([
            "perf-lock",
            "--runs",
            "1",
            "--budget-ms",
            "60000",
            "--falcon-bin",
        ]);
        let output = command
            .arg(&binary)
            .args(skip_build.then_some("--skip-build"))
            .arg("--corpus")
            .arg(directory.path())
            .env("CARGO_CMD", directory.path().join("missing-cargo"))
            .env("PERF_MARKER", &marker)
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read_to_string(marker).unwrap(), "ran\nran\nran\nran\n");
}
