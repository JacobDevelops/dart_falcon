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

fn workflow(path: &str) -> serde_yaml::Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    serde_yaml::from_str(&fs::read_to_string(root.join(path)).unwrap()).unwrap()
}

fn step_script(workflow: &serde_yaml::Value, job: &str, name: &str) -> String {
    workflow["jobs"][job]["steps"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|step| step["name"].as_str() == Some(name))
        .unwrap()["run"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn dispatch_resolves_versions_without_moving_existing_tags() {
    let workflow = workflow(".github/workflows/release.yml");
    assert_eq!(
        workflow["on"]["workflow_dispatch"]["inputs"]["tag"]["required"],
        false
    );
    let script = step_script(&workflow, "resolve", "Resolve release version");
    let checkout = &workflow["jobs"]["resolve"]["steps"][0]["with"];
    assert_eq!(checkout["fetch-depth"], 0);
    assert_eq!(
        checkout["ref"],
        "${{ github.event.pull_request.merge_commit_sha || (github.event.inputs.tag && format('refs/tags/{0}', github.event.inputs.tag)) || github.ref }}"
    );
    for (event, input, existing, succeeds) in [
        ("workflow_dispatch", "", "", true),
        ("workflow_dispatch", "v1.2.3", "head", true),
        ("pull_request", "", "", true),
        ("push", "v1.2.3", "head", true),
        ("workflow_dispatch", "v9.9.9", "", false),
        ("push", "v9.9.9", "", false),
        ("workflow_dispatch", "", "other", false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"
cargo() {{ printf '%s\n' '{{"packages":[{{"name":"dart_falcon","version":"1.2.3"}}]}}'; }}
git() {{
  case "$1" in
    show-ref) test -n "$EXISTING" ;;
    rev-parse) if [ "$2" = HEAD ]; then echo head; else echo "$EXISTING"; fi ;;
    *) return 1 ;;
  esac
}}
{script}
"#
            ))
            .env("INPUT_TAG", input)
            .env("GITHUB_EVENT_NAME", event)
            .env("EXISTING", existing)
            .env("GITHUB_OUTPUT", temp.path().join("output"))
            .output()
            .unwrap();
        assert_eq!(output.status.success(), succeeds, "{output:?}");
        if succeeds {
            assert_eq!(
                fs::read_to_string(temp.path().join("output")).unwrap(),
                "tag=v1.2.3\nref=head\n"
            );
        }
    }
}

#[test]
fn notarisation_reuses_history_without_waiting() {
    let workflow = workflow(".github/workflows/release.yml");
    let script = step_script(&workflow, "notarise", "Sign and notarise macOS binary");
    assert!(!script.contains("--wait"));
    for (status, published) in [
        ("", false),
        ("Accepted", false),
        ("Accepted", true),
        ("In Progress", false),
        ("Invalid", false),
        ("Rejected", false),
        ("Different Commit", false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let name = "falcon-1.2.3-aarch64-darwin-abc123.zip";
        let history = if status.is_empty() {
            serde_json::json!({"history": []})
        } else {
            serde_json::json!({"history": [{"id": "existing", "name": name, "status": status}]})
        };
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"
mkdir -p stage
echo binary > stage/falcon
gh() {{
  printf '%s\n' "$*" >> "$RUNNER_TEMP/gh-calls"
  if [ "$1" = api ]; then
    if [ "$PUBLISHED" = true ]; then
      echo '[[{{"tag_name":"v1.2.3","draft":false,"assets":[{{"name":"falcon-1.2.3-aarch64-darwin.tar.gz"}}]}}]]'
    elif [ -z "$STATUS" ]; then echo '[[]]'; else
      echo '[[{{"tag_name":"v1.2.3","draft":true,"target_commitish":"head","assets":[{{"name":"falcon-1.2.3-aarch64-darwin.signed"}}]}}]]'
    fi
  elif [ "$2" = download ]; then
    if [ "$PUBLISHED" = true ]; then
      mkdir -p saved
      echo 'signed binary' > saved/falcon
      command tar -czf "$RUNNER_TEMP/falcon-1.2.3-aarch64-darwin.tar.gz" -C saved falcon
    else
      echo 'signed binary' > "$RUNNER_TEMP/falcon-1.2.3-aarch64-darwin.signed"
    fi
  fi
}}
security() {{
  if [ "$1" = find-identity ]; then echo '1) hash "Developer ID Application: Test"'; fi
}}
openssl() {{ echo password; }}
base64() {{ cat; }}
codesign() {{
  printf '%s\n' "$*" >> "$RUNNER_TEMP/codesign-calls"
  if [ "$1" = -dvvv ]; then echo CDHash=abc123 >&2; fi
}}
ditto() {{ :; }}
tar() {{ printf '%s\n' "$*" >> "$RUNNER_TEMP/tar-calls"; command tar "$@"; }}
xcrun() {{
  printf '%s\n' "$*" >> "$RUNNER_TEMP/calls"
  case "$2" in
    history) printf '%s\n' "$HISTORY" ;;
    submit) printf '%s\n' '{{"id":"new"}}' ;;
    info) printf '%s\n' '{{"id":"new","status":"In Progress"}}' ;;
    *) return 1 ;;
  esac
}}
{script}
"#
            ))
            .current_dir(temp.path())
            .env("STATUS", status)
            .env("PUBLISHED", published.to_string())
            .env("GH_REPO", "owner/repo")
            .env("RELEASE_REF", if status == "Different Commit" { "other" } else { "head" })
            .env("RUNNER_TEMP", temp.path())
            .env("GITHUB_STEP_SUMMARY", temp.path().join("summary"))
            .env("RELEASE_TAG", "v1.2.3")
            .env("PLATFORM", "aarch64-darwin")
            .env("HISTORY", history.to_string())
            .env("APPLE_CERTIFICATE_P12", "certificate")
            .env("APPLE_CERTIFICATE_PASSWORD", "password")
            .env("APPLE_ID", "test@example.com")
            .env("APPLE_TEAM_ID", "team")
            .env("APPLE_APP_PASSWORD", "password")
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            status == "Accepted",
            "{status}: {output:?}"
        );
        if status == "Different Commit" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("rerun the original ref"));
            assert!(!temp.path().join("calls").exists());
            continue;
        }
        assert_eq!(temp.path().join("tar-calls").exists(), published);
        let calls = fs::read_to_string(temp.path().join("calls")).unwrap();
        assert_eq!(
            calls.contains("notarytool submit"),
            status.is_empty(),
            "{calls}"
        );
        let codesign = fs::read_to_string(temp.path().join("codesign-calls")).unwrap();
        assert_eq!(
            codesign.contains("--force"),
            status.is_empty(),
            "{codesign}"
        );
        if !status.is_empty() {
            assert_eq!(
                fs::read_to_string(temp.path().join("stage/falcon")).unwrap(),
                "signed binary\n"
            );
        }
        let summary = fs::read_to_string(temp.path().join("summary")).unwrap();
        assert!(summary.contains(name), "{summary}");
        assert!(summary.contains(if status.is_empty() { "new" } else { "existing" }));
    }
}

#[test]
fn publishing_checks_existing_versions_and_preserves_tag_scope() {
    let workflow = workflow(".github/workflows/publish.yml");
    let script = step_script(&workflow, "pub", "Check published version");
    assert!(
        step_script(&workflow, "pub", "Require matching version tag").contains("GITHUB_REF_TYPE")
    );
    for (status, succeeds, exists) in [
        ("200", true, "true"),
        ("404", true, "false"),
        ("500", false, ""),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!("curl() {{ echo \"$STATUS\"; }}\n{script}"))
            .env("STATUS", status)
            .env("GITHUB_REF_NAME", "v1.2.3")
            .env("GITHUB_OUTPUT", temp.path().join("output"))
            .output()
            .unwrap();
        assert_eq!(output.status.success(), succeeds, "{output:?}");
        if succeeds {
            assert_eq!(
                fs::read_to_string(temp.path().join("output")).unwrap(),
                format!("exists={exists}\n")
            );
        }
    }
    for step in workflow["jobs"]["pub"]["steps"].as_sequence().unwrap() {
        if step["uses"]
            .as_str()
            .is_some_and(|uses| uses.starts_with("dart-lang/"))
            || step["run"]
                .as_str()
                .is_some_and(|run| run.starts_with("dart pub"))
        {
            assert_eq!(step["if"], "steps.published.outputs.exists == 'false'");
        }
    }
}

#[test]
fn publication_removes_saved_binary_before_publishing_and_dispatching() {
    let workflow = workflow(".github/workflows/release.yml");
    assert_eq!(workflow["jobs"]["package"]["needs"][2], "notarise");
    assert_eq!(workflow["jobs"]["release"]["needs"][1], "package");
    assert_eq!(workflow["jobs"]["homebrew"]["needs"][1], "release");
    let steps = workflow["jobs"]["release"]["steps"].as_sequence().unwrap();
    let action = steps
        .iter()
        .position(|step| step["id"] == "github-release")
        .unwrap();
    let publish = steps
        .iter()
        .position(|step| step["name"] == "Publish accepted release")
        .unwrap();
    let dispatch = steps
        .iter()
        .position(|step| step["name"] == "Dispatch package publishing")
        .unwrap();
    assert!(action < publish && publish < dispatch);
    assert_eq!(
        steps[action]["with"]["draft"],
        "${{ steps.state.outputs.draft == 'true' }}"
    );
    assert!(
        steps[dispatch]["run"]
            .as_str()
            .unwrap()
            .contains("--ref \"$RELEASE_TAG\"")
    );
    let script = step_script(&workflow, "release", "Publish accepted release");
    for remote in ["", "head", "other", "annotated"] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(r#"
git() {{
  if [ "$1" = rev-parse ]; then echo head;
  elif [ "$REMOTE" = annotated ]; then printf 'tag-object refs/tags/v1.2.3\nhead refs/tags/v1.2.3^{{}}\n';
  elif [ -n "$REMOTE" ]; then printf '%s refs/tags/v1.2.3\n' "$REMOTE"; fi
}}
gh() {{
  if [ "$2" = --paginate ]; then
    echo '[[{{"name":"falcon-1.2.3-aarch64-darwin.signed","id":10}},{{"name":"SHA256SUMS","id":11}}]]'
  else
    printf '%s\n' "$*" >> "$CALLS"
  fi
}}
{script}
"#))
            .env("GITHUB_REPOSITORY", "owner/repo")
            .env("RELEASE_TAG", "v1.2.3")
            .env("REMOTE", remote)
            .env("RELEASE_ID", "123")
            .env("CALLS", temp.path().join("calls"))
            .output()
            .unwrap();
        assert_eq!(output.status.success(), remote != "other", "{output:?}");
        if remote == "other" {
            assert!(!temp.path().join("calls").exists());
            continue;
        }
        let calls = fs::read_to_string(temp.path().join("calls")).unwrap();
        let calls: Vec<_> = calls.lines().collect();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].contains("DELETE repos/owner/repo/releases/assets/10"));
        assert!(calls[1].contains("PATCH repos/owner/repo/releases/123 -F draft=false"));
    }
}
