#![cfg(unix)]

use std::{fs, process::Command};

const CREDENTIALS: [(&str, &str); 8] = [
    ("HAS_APPLE_CERTIFICATE", "secrets.APPLE_CERTIFICATE_P12"),
    (
        "HAS_APPLE_CERTIFICATE_PASSWORD",
        "secrets.APPLE_CERTIFICATE_PASSWORD",
    ),
    ("HAS_APPLE_API_ISSUER_ID", "secrets.APPLE_API_ISSUER_ID"),
    ("HAS_APPLE_API_KEY_ID", "secrets.APPLE_API_KEY_ID"),
    ("HAS_APPLE_API_KEY_P8", "secrets.APPLE_API_KEY_P8"),
    ("HAS_APPLE_TEAM_ID", "secrets.APPLE_TEAM_ID"),
    ("HAS_MINISIGN_KEY", "secrets.MINISIGN_SECRET_KEY"),
    ("HAS_TAP_TOKEN", "secrets.HOMEBREW_TAP_TOKEN"),
];

fn check_credentials(configured: [bool; CREDENTIALS.len()]) {
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
        "${{ github.event.pull_request.merge_commit_sha || github.event.inputs.commit || (github.event.inputs.tag && format('refs/tags/{0}', github.event.inputs.tag)) || github.sha }}"
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
            .env("INPUT_COMMIT", "")
            .env("DEFAULT_BRANCH", "main")
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
    for (statuses, published, expected) in [
        ("", false, "In Progress"),
        ("Accepted", false, "Accepted"),
        ("Accepted", true, "Accepted"),
        ("In Progress", false, "In Progress"),
        ("Invalid", false, "Invalid"),
        ("Rejected", false, "Rejected"),
        ("In Progress,Accepted", false, "Accepted"),
        ("Accepted,In Progress", false, "Accepted"),
        ("Accepted,Invalid", false, "Invalid"),
        ("Rejected,Accepted", false, "Rejected"),
        ("Different Commit", false, ""),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let name = "falcon-1.2.3-aarch64-darwin-ba7816bf8f01cfea414140de5dae2223b00361a3.zip";
        let existing = "11111111-1111-1111-1111-111111111111";
        let submitted = "22222222-2222-2222-2222-222222222222";
        let mut history = String::from("id date name type status\n");
        history.push_str(&format!(
            "{existing} 2026-10-01 unrelated.zip zip invalid\n"
        ));
        if !statuses.is_empty() {
            for status in statuses.split(',') {
                let cli_status = status.to_ascii_lowercase();
                history.push_str(&format!("{existing} 2026-10-01 {name} zip {cli_status}\n"));
            }
        }
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
    elif [ -z "$STATUSES" ]; then echo '[[]]'; else
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
base64() {{ cat; }}
INPUT_COMMIT="${{INPUT_COMMIT:-}}"
zip() {{ printf '%s\n' "$*" >> "$RUNNER_TEMP/zip-calls"; }}
tar() {{ printf '%s\n' "$*" >> "$RUNNER_TEMP/tar-calls"; command tar "$@"; }}
rcodesign() {{
  printf '%s\n' "$*" >> "$RUNNER_TEMP/calls"
  case "$1" in
    sign|verify|encode-app-store-connect-api-key) ;;
    extract) printf abc ;;
    notary-list) printf '%s\n' "$HISTORY" ;;
    notary-submit) printf 'created submission ID: {submitted}\n' >&2 ;;
    notary-log) printf 'rejection details\n' ;;
    *) return 1 ;;
  esac
}}
{script}
                "#
            ))
            .current_dir(temp.path())
            .env("STATUSES", statuses)
            .env("PUBLISHED", published.to_string())
            .env("GH_REPO", "owner/repo")
            .env("RELEASE_REF", if statuses == "Different Commit" { "other" } else { "head" })
            .env("RUNNER_TEMP", temp.path())
            .env("GITHUB_OUTPUT", temp.path().join("output"))
            .env("GITHUB_STEP_SUMMARY", temp.path().join("summary"))
            .env("RELEASE_TAG", "v1.2.3")
            .env("PLATFORM", "aarch64-darwin")
            .env("HISTORY", history)
            .env("APPLE_CERTIFICATE_P12", "certificate")
            .env("APPLE_CERTIFICATE_PASSWORD", "password")
            .env("APPLE_API_ISSUER_ID", "issuer")
            .env("APPLE_API_KEY_ID", "key")
            .env("APPLE_API_KEY_P8", "private key")
            .env("APPLE_TEAM_ID", "team")
            .output()
            .unwrap();
        assert_notarisation_result(
            temp.path(),
            &output,
            statuses,
            published,
            expected,
            (name, existing, submitted),
        );
    }
}

fn assert_notarisation_result(
    root: &std::path::Path,
    output: &std::process::Output,
    statuses: &str,
    published: bool,
    expected: &str,
    submission: (&str, &str, &str),
) {
    let (name, existing, submitted) = submission;
    assert_eq!(
        output.status.success(),
        matches!(expected, "Accepted" | "In Progress"),
        "{statuses}: {output:?}"
    );
    if statuses == "Different Commit" {
        assert!(String::from_utf8_lossy(&output.stderr).contains("rerun the original ref"));
        assert!(!root.join("calls").exists());
        return;
    }
    assert_eq!(root.join("tar-calls").exists(), published);
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert_eq!(
        calls.lines().any(|line| line.starts_with("notary-submit ")),
        statuses.is_empty(),
        "{calls}"
    );
    assert_eq!(
        calls.lines().any(|line| line.starts_with("sign ")),
        statuses.is_empty(),
        "{calls}"
    );
    assert!(
        calls.lines().any(|line| line.starts_with("verify ")),
        "{calls}"
    );
    assert_eq!(root.join("zip-calls").exists(), statuses.is_empty());
    if !statuses.is_empty() {
        assert_eq!(
            fs::read_to_string(root.join("stage/falcon")).unwrap(),
            "signed binary\n"
        );
    }
    let summary = fs::read_to_string(root.join("summary")).unwrap();
    let id = if statuses.is_empty() {
        submitted
    } else {
        existing
    };
    assert!(summary.contains(name), "{summary}");
    assert!(summary.contains(id), "{summary}");
    if output.status.success() {
        let outputs = fs::read_to_string(root.join("output")).unwrap();
        for field in [
            format!("id={id}"),
            format!("name={name}"),
            format!("status={expected}"),
        ] {
            assert!(outputs.lines().any(|line| line == field), "{outputs}");
        }
    }
    let gh_calls = fs::read_to_string(root.join("gh-calls")).unwrap();
    assert!(!gh_calls.contains("draft=false"), "{gh_calls}");
    assert!(!gh_calls.contains("publish.yml"), "{gh_calls}");
    if statuses.is_empty() {
        assert!(gh_calls.contains("--draft --target head"), "{gh_calls}");
        assert!(gh_calls.contains(".signed"), "{gh_calls}");
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

#[test]
fn continuation_commit_must_be_immutable_and_on_the_default_branch() {
    let workflow = workflow(".github/workflows/release.yml");
    assert_eq!(
        workflow["on"]["workflow_dispatch"]["inputs"]["commit"]["required"],
        false
    );
    let script = step_script(&workflow, "resolve", "Resolve release version");
    let commit = "1234567890abcdef1234567890abcdef12345678";
    for (input, head, ancestor, succeeds) in [
        (commit, commit, true, true),
        ("main", commit, true, false),
        (commit, "other", true, false),
        (commit, commit, false, false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"
cargo() {{ printf '%s\n' '{{"packages":[{{"name":"dart_falcon","version":"1.2.3"}}]}}'; }}
git() {{
  case "$1" in
    rev-parse) echo "$HEAD" ;;
    merge-base) [ "$*" = "merge-base --is-ancestor HEAD origin/main" ] && [ "$ANCESTOR" = true ] ;;
    show-ref) return 1 ;;
    *) return 1 ;;
  esac
}}
{script}
"#
            ))
            .env("INPUT_COMMIT", input)
            .env("INPUT_TAG", "v1.2.3")
            .env("DEFAULT_BRANCH", "main")
            .env("HEAD", head)
            .env("ANCESTOR", ancestor.to_string())
            .env("GITHUB_OUTPUT", temp.path().join("output"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            succeeds,
            "{input}, {head}, {ancestor}: {output:?}"
        );
        if succeeds {
            assert_eq!(
                fs::read_to_string(temp.path().join("output")).unwrap(),
                format!("tag=v1.2.3\nref={commit}\n")
            );
        }
    }
}

#[test]
fn notarisation_wait_is_separate_bounded_and_least_privileged() {
    let release = workflow(".github/workflows/release.yml");
    let polling = workflow(".github/workflows/notarisation.yml");
    for workflow in [&release, &polling] {
        assert_eq!(workflow["permissions"]["contents"], "read");
        for (name, job) in workflow["jobs"].as_mapping().unwrap() {
            if job.get("uses").is_none() {
                assert_eq!(job["runs-on"], "ubuntu-latest", "{name:?}");
            }
            if job["permissions"]["contents"] == "write" {
                assert!(
                    matches!(
                        name.as_str(),
                        Some("notarise" | "release" | "await-notarisation" | "check")
                    ),
                    "{name:?}"
                );
            }
        }
    }
    assert_eq!(
        release["jobs"]["package"]["if"],
        "${{ !cancelled() && needs.notarise.result == 'success' && (needs.notarise.outputs.status == 'Accepted' || needs.await-notarisation.outputs.status == 'Accepted') }}"
    );
    assert_eq!(release["jobs"]["package"]["needs"][3], "await-notarisation");
    for (job, dependency) in [("release", "package"), ("homebrew", "release")] {
        assert_eq!(
            release["jobs"][job]["if"],
            format!(
                "${{{{ !cancelled() && needs.resolve.result == 'success' && needs.{dependency}.result == 'success' }}}}"
            )
        );
    }
    for field in ["id", "name", "status"] {
        assert_eq!(
            release["jobs"]["notarise"]["outputs"][field],
            format!("${{{{ steps.notary.outputs.{field} }}}}")
        );
    }
    assert_eq!(polling["jobs"]["wait"]["environment"], "notarisation");
    assert_eq!(polling["jobs"]["check"]["needs"], "wait");
    assert_eq!(polling["jobs"]["wait"]["steps"][0]["run"], ":");
    let poll = step_script(&polling, "check", "Check submission and continue release");
    assert!(!poll.contains("--wait"));
    assert!(!poll.contains("sleep "));
    let dispatch = &release["jobs"]["await-notarisation"];
    assert_eq!(
        dispatch["if"],
        "needs.notarise.outputs.status == 'In Progress'"
    );
    assert_eq!(dispatch["permissions"].as_mapping().unwrap().len(), 2);
    assert_eq!(dispatch["permissions"]["contents"], "write");
    assert_eq!(dispatch["uses"], "./.github/workflows/notarisation.yml");
    assert_eq!(dispatch["with"]["attempt"], "1");
    assert_eq!(dispatch["with"]["continue-in-caller"], true);
    assert_eq!(
        polling["on"]["workflow_call"]["outputs"]["status"]["value"],
        "${{ jobs.check.outputs.status }}"
    );
    assert_eq!(dispatch["permissions"]["actions"], "write");
    let check_permissions = &polling["jobs"]["check"]["permissions"];
    assert_eq!(check_permissions["actions"], "write");
    assert_eq!(check_permissions["contents"], "write");
    for (workflow, job) in [(&release, "notarise"), (&polling, "check")] {
        let install = step_script(workflow, job, "Install pinned Apple signing tool");
        for required in [
            "apple-codesign-0.29.0-x86_64-unknown-linux-musl.tar.gz",
            "dbe85cedd8ee4217b64e9a0e4c2aef92ab8bcaaa41f20bde99781ff02e600002",
            "sha256sum --check",
        ] {
            assert!(install.contains(required), "{install}");
        }
    }
    let sign = step_script(&release, "notarise", "Sign and notarise macOS binary");
    for required in [
        "--p12-file",
        "--p12-password-file",
        "--binary-identifier falcon",
        "--team-name",
        "--digest sha256",
        "--code-signature-flags runtime",
        "--for-notarization",
        "--timestamp-url http://timestamp.apple.com/ts01",
        "extract code-directory-raw",
        "sha256sum",
        "${cdhash:0:40}",
        "encode-app-store-connect-api-key",
        "notary-list --api-key-file",
    ] {
        assert!(sign.contains(required), "{required}: {sign}");
    }
    for (input, expression) in [
        ("release-ref", "${{ needs.resolve.outputs.ref }}"),
        ("release-tag", "${{ needs.resolve.outputs.tag }}"),
        ("submission-id", "${{ needs.notarise.outputs.id }}"),
        ("submission-name", "${{ needs.notarise.outputs.name }}"),
    ] {
        assert_eq!(dispatch["with"][input], expression);
    }
}

#[test]
fn polling_continues_only_accepted_submissions_and_stops_after_six_checks() {
    let workflow = workflow(".github/workflows/notarisation.yml");
    let script = step_script(&workflow, "check", "Check submission and continue release");
    let commit = "1234567890abcdef1234567890abcdef12345678";
    let id = "11111111-1111-1111-1111-111111111111";
    for (status, attempt, target, caller, succeeds, dispatch) in [
        ("Accepted", "1", commit, false, true, "release.yml"),
        ("Accepted", "6", commit, false, true, "release.yml"),
        ("Accepted", "1", commit, true, true, ""),
        ("Accepted", "6", commit, true, true, ""),
        ("In Progress", "1", commit, false, true, "notarisation.yml"),
        ("In Progress", "1", commit, true, true, "notarisation.yml"),
        ("In Progress", "5", commit, false, true, "notarisation.yml"),
        ("In Progress", "6", commit, false, false, ""),
        ("In Progress", "6", commit, true, false, ""),
        ("Accepted", "7", commit, false, false, ""),
        ("Accepted", "0", commit, false, false, ""),
        ("Invalid", "1", commit, false, false, ""),
        ("Rejected", "1", commit, false, false, ""),
        ("Unknown", "1", commit, false, false, ""),
        ("", "1", commit, false, false, ""),
        ("Accepted", "1", "other", false, false, ""),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"
gh() {{
  if [ "$1" = api ]; then
    printf '[[{{"tag_name":"v1.2.3","draft":true,"target_commitish":"%s"}}]]\n' "$TARGET"
  else
    printf '%s\n' "$*" >> "$RUNNER_TEMP/dispatch"
  fi
}}
rcodesign() {{
  printf '%s\n' "$*" >> "$RUNNER_TEMP/calls"
  case "$1" in
    encode-app-store-connect-api-key) ;;
    notary-list)
      printf 'id date name type status\n'
      printf 'another-id 2026-10-01 submission.zip zip rejected\n'
      printf '{id} 2026-10-01 another-name.zip zip invalid\n'
      if [ -n "$STATUS" ]; then printf '{id} 2026-10-01 submission.zip zip %s\n' "$STATUS"; fi
      ;;
    notary-log) echo 'Apple rejection details' ;;
    *) return 1 ;;
  esac
}}
{script}
"#
            ))
            .current_dir(temp.path())
            .env("RUNNER_TEMP", temp.path())
            .env("GITHUB_OUTPUT", temp.path().join("output"))
            .env("GITHUB_STEP_SUMMARY", temp.path().join("summary"))
            .env("GH_REPO", "owner/repo")
            .env("DEFAULT_BRANCH", "main")
            .env("RELEASE_REF", commit)
            .env("RELEASE_TAG", "v1.2.3")
            .env("SUBMISSION_ID", id)
            .env("SUBMISSION_NAME", "submission.zip")
            .env("ATTEMPT", attempt)
            .env("CONTINUE_IN_CALLER", caller.to_string())
            .env("STATUS", status.to_ascii_lowercase())
            .env("TARGET", target)
            .env("APPLE_API_ISSUER_ID", "issuer")
            .env("APPLE_API_KEY_ID", "key")
            .env("APPLE_API_KEY_P8", "private key")
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            succeeds,
            "{status}, {attempt}, {target}: {output:?}"
        );
        if succeeds {
            let outputs = fs::read_to_string(temp.path().join("output")).unwrap();
            for field in [format!("id={id}"), format!("status={status}")] {
                assert!(outputs.lines().any(|line| line == field), "{outputs}");
            }
        }
        assert_polling_dispatch(temp.path(), &output, dispatch, commit, id, attempt);
        if matches!(status, "Invalid" | "Rejected") {
            assert!(String::from_utf8_lossy(&output.stdout).contains("Apple rejection details"));
            let calls = fs::read_to_string(temp.path().join("calls")).unwrap();
            assert!(
                calls.lines().any(|line| line.starts_with("notary-log ")),
                "{calls}"
            );
        }
        if status == "In Progress" && attempt == "6" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("manually rerun"));
        }
    }
}

fn assert_polling_dispatch(
    root: &std::path::Path,
    output: &std::process::Output,
    dispatch: &str,
    commit: &str,
    id: &str,
    attempt: &str,
) {
    let dispatched = root.join("dispatch");
    assert_eq!(dispatched.exists(), !dispatch.is_empty(), "{output:?}");
    if !dispatch.is_empty() {
        let calls = fs::read_to_string(dispatched).unwrap();
        assert_eq!(calls.lines().count(), 1, "{calls}");
        assert!(
            calls.contains(&format!("workflow run {dispatch} --ref main")),
            "{calls}"
        );
        if dispatch == "release.yml" {
            assert!(
                calls.contains(&format!("-f commit={commit} -f tag=v1.2.3")),
                "{calls}"
            );
        } else {
            for field in [
                format!("-f release-ref={commit}"),
                "-f release-tag=v1.2.3".to_owned(),
                format!("-f submission-id={id}"),
                "-f submission-name=submission.zip".to_owned(),
                format!("-f attempt={}", attempt.parse::<u8>().unwrap() + 1),
            ] {
                assert!(calls.contains(&field), "{calls}");
            }
        }
    }
}

#[test]
fn continuation_requires_the_recorded_release_commit_before_signing() {
    let workflow = workflow(".github/workflows/release.yml");
    let script = step_script(&workflow, "notarise", "Sign and notarise macOS binary");
    for release in [
        "[[]]",
        r#"[[{"tag_name":"v1.2.3","draft":true,"target_commitish":"other","assets":[]}]]"#,
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"
mkdir stage
echo binary > stage/falcon
gh() {{ printf '%s\n' "$RELEASE"; }}
rcodesign() {{ echo 'must not sign or query Apple' > "$RUNNER_TEMP/unexpected"; return 1; }}
{script}
"#
            ))
            .current_dir(temp.path())
            .env("RUNNER_TEMP", temp.path())
            .env("RELEASE", release)
            .env("RELEASE_TAG", "v1.2.3")
            .env("PLATFORM", "aarch64-darwin")
            .env("GH_REPO", "owner/repo")
            .env("INPUT_COMMIT", "1234567890abcdef1234567890abcdef12345678")
            .env("RELEASE_REF", "1234567890abcdef1234567890abcdef12345678")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("recorded draft release commit"));
        assert!(!temp.path().join("unexpected").exists());
    }
}
