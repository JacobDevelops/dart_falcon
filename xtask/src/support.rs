use crate::workspace_root;
use falcon_rules::meta::RuleMeta;
use serde_json::{Map, Value, json};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn prepare_checkout(
    checkout: &Path,
    repository: &str,
    revision: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let run = |command: &mut Command| -> Result<String, Box<dyn std::error::Error>> {
        let output = command.output()?;
        if !output.status.success() {
            return Err(format!("{command:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    };
    fs::create_dir_all(checkout)?;
    if !checkout.join(".git").exists() {
        run(Command::new("git").arg("init").arg(checkout))?;
    }
    let actual = run(Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["rev-parse", "HEAD"]));
    if actual.as_deref().ok() != Some(revision) {
        run(Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["fetch", "--depth", "1", repository, revision]))?;
        run(Command::new("git").arg("-C").arg(checkout).args([
            "checkout",
            "--force",
            "--detach",
            "FETCH_HEAD",
        ]))?;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct DiagnosticJson {
    pub(crate) rule: String,
    pub(crate) message: String,
    pub(crate) line: Option<usize>,
    pub(crate) col: Option<usize>,
    #[serde(default)]
    pub(crate) file_path: String,
}

impl DiagnosticJson {
    pub(crate) fn resolved_position(&self) -> Result<(usize, usize), String> {
        match (self.line, self.col) {
            (Some(line), Some(col)) if line > 0 && col > 0 => Ok((line, col)),
            _ => Err(format!(
                "diagnostic `{}` in {} has no resolved source position: {}",
                self.rule, self.file_path, self.message
            )),
        }
    }
}

#[path = "../../crates/falcon_rules/tests/support/sdk_fixture.rs"]
mod sdk_fixture;

/// Bundled listed corpus fixtures always use parsed dependency sources and
/// unlisted ones run without an SDK, independently of the developer machine's
/// installed SDK.
pub(crate) struct FalconRunner {
    pub(crate) binary: String,
    pub(crate) fallback_sdk: Option<PathBuf>,
}

impl FalconRunner {
    pub(crate) fn new(binary: PathBuf, workspace: &Path, corpus: &Path) -> Self {
        let fallback_sdk = corpus_fixture_sdk(workspace, corpus);
        // Isolated bundled runs remove the child's PATH, so bare names resolve against ours.
        let binary = if fallback_sdk.is_some() {
            resolve_binary(
                binary,
                std::env::var_os("PATH"),
                &std::env::current_dir().unwrap_or_default(),
            )
        } else {
            binary
        };
        Self {
            binary: binary.to_string_lossy().into_owned(),
            fallback_sdk,
        }
    }

    fn sdk_for(&self, file: &Path) -> Option<&Path> {
        self.fallback_sdk.as_deref().filter(|_| {
            if file.is_dir() {
                walkdir::WalkDir::new(file).into_iter().any(|entry| {
                    let entry = entry
                        .unwrap_or_else(|error| panic!("cannot walk {}: {error}", file.display()));
                    entry.file_type().is_file() && sdk_fixture::requires_sdk(entry.path())
                })
            } else {
                sdk_fixture::requires_sdk(file)
            }
        })
    }

    pub(crate) fn run(
        &self,
        file: &Path,
        config: Option<&Path>,
    ) -> Result<Vec<DiagnosticJson>, String> {
        // Stage the parsed SDK, dependency sources and package metadata together,
        // using the same fixture builder as the in-process corpus harness.
        let staged = self.sdk_for(file).map(|_| {
            let directory = if file.is_dir() {
                file
            } else {
                sdk_fixture::source_directory(file)
            };
            (sdk_fixture::stage_project(directory), directory)
        });
        let analyzed_file = staged.as_ref().map_or_else(
            || file.to_path_buf(),
            |(project, directory)| sdk_fixture::staged_file(project.path(), directory, file),
        );
        let staged_config = match (&staged, config) {
            (Some((project, directory)), Some(config)) => Some(write_staged_config(
                config,
                project.path(),
                directory,
                file,
            )?),
            _ => None,
        };
        let config = staged_config.as_deref().or(config);
        let mut cmd = Command::new(&self.binary);
        cmd.args(["check", "--format", "json"]);
        cmd.arg(&analyzed_file);
        if let Some(config_path) = config {
            cmd.args(["--config", config_path.to_str().unwrap_or("")]);
        }
        if let Some((project, _)) = &staged {
            cmd.env_remove("FLUTTER_ROOT").env_remove("PATH");
            cmd.env(
                "DART_SDK",
                project.path().join("flutter/bin/cache/dart-sdk"),
            );
        } else if self.fallback_sdk.is_some() {
            // Match the SDK-free in-process harness so output is machine-independent.
            for variable in ["DART_SDK", "FLUTTER_ROOT", "PATH"] {
                cmd.env_remove(variable);
            }
        }
        let output = cmd
            .env("RUST_LOG", "error")
            .output()
            .map_err(|e| format!("Failed to run falcon binary '{}': {}", self.binary, e))?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(format!(
                "falcon exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        let mut diagnostics =
            serde_json::from_str::<Vec<DiagnosticJson>>(trimmed).map_err(|e| {
                format!(
                    "JSON parse error: {}\nOutput was: {}\nStderr: {}",
                    e,
                    &trimmed[..trimmed.len().min(200)],
                    String::from_utf8_lossy(&output.stderr)
                )
            })?;
        if let Some((project, directory)) = &staged {
            let staged_root = falcon_analyze::canonical_or_lexical(&sdk_fixture::staged_directory(
                project.path(),
                directory,
            ));
            for diagnostic in &mut diagnostics {
                if let Ok(relative) = Path::new(&diagnostic.file_path).strip_prefix(&staged_root) {
                    diagnostic.file_path = directory.join(relative).to_string_lossy().into_owned();
                }
            }
        }
        Ok(diagnostics)
    }
}

/// Preserve original source-path selection when SDK staging changes file roots.
/// Directory targets retain independent override matching for each source file.
fn write_staged_config(
    original_config: &Path,
    project: &Path,
    source_directory: &Path,
    target: &Path,
) -> Result<PathBuf, String> {
    let mut config = falcon_rules::load_config(original_config)
        .map_err(|error| error.to_string())?
        .config;
    let files = if target.is_dir() {
        recursive_files(target)
            .into_iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "dart")
            })
            .collect()
    } else {
        vec![target.to_path_buf()]
    };
    let paths: Vec<_> = files
        .iter()
        .map(|original| {
            let staged = sdk_fixture::staged_file(project, source_directory, original);
            (
                original.to_string_lossy().into_owned(),
                glob::Pattern::escape(&staged.to_string_lossy()),
            )
        })
        .collect();
    let mut included: Vec<_> = paths
        .iter()
        .filter(|(original, _)| config.files.includes.admits(original))
        .map(|(_, staged)| staged.clone())
        .collect();
    if included.is_empty() {
        included.push("!**".to_string());
    }
    config.files.includes = included.into();
    config.overrides.retain_mut(|entry| {
        let included: Vec<_> = paths
            .iter()
            .filter(|(original, _)| entry.matches(original))
            .map(|(_, staged)| staged.clone())
            .collect();
        if included.is_empty() {
            return false;
        }
        entry.includes = included.into();
        true
    });
    let path = project.join("falcon.json");
    let contents = serde_json::to_vec(&config).map_err(|error| error.to_string())?;
    fs::write(&path, contents)
        .map_err(|error| format!("cannot write staged config {}: {error}", path.display()))?;
    Ok(path)
}

fn corpus_fixture_sdk(workspace: &Path, corpus: &Path) -> Option<PathBuf> {
    let bundled = workspace.join("crates/falcon_rules/tests/corpus");
    (falcon_analyze::canonical_or_lexical(corpus) == falcon_analyze::canonical_or_lexical(&bundled))
        .then(|| workspace.join("crates/falcon_rules/tests/support/sdk"))
}

/// Resolve a bare executable name against `search_path` like process lookup,
/// skipping non-executable matches; paths are kept as given.
fn resolve_binary(
    binary: PathBuf,
    search_path: Option<OsString>,
    current_directory: &Path,
) -> PathBuf {
    if binary.components().count() != 1 {
        return binary;
    }
    search_path
        .and_then(|search_path| {
            std::env::split_paths(&search_path)
                .map(|directory| {
                    // Empty and relative entries mean our working directory; anchor them.
                    let candidate = current_directory.join(directory).join(&binary);
                    // Windows lookup appends `.exe` to extensionless names; Unix has no suffix.
                    if binary.extension().is_none() {
                        candidate.with_extension(std::env::consts::EXE_EXTENSION)
                    } else {
                        candidate
                    }
                })
                .find(|candidate| is_executable(candidate))
        })
        .unwrap_or(binary)
}

fn is_executable(candidate: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        candidate
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        candidate.is_file()
    }
}

pub(crate) fn recursive_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("cannot walk {}: {error}", root.display()))
        })
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect();
    files.sort_unstable();
    files
}

pub(crate) fn build_falcon(release: bool) -> PathBuf {
    let workspace = workspace_root();
    eprintln!(
        "{}",
        if release {
            "building release falcon binary..."
        } else {
            "building falcon binary (debug)..."
        }
    );
    let cargo = std::env::var_os("CARGO_CMD").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(&cargo);
    command.args(["build", "--locked", "-p", "dart_falcon", "--bin", "falcon"]);
    if release {
        command.arg("--release");
    }
    let status = command
        .current_dir(&workspace)
        .status()
        .expect("failed to spawn cargo build");
    if !status.success() {
        eprintln!(
            "{}",
            if release {
                "error: release build failed"
            } else {
                "error: falcon build failed"
            }
        );
        std::process::exit(1);
    }
    let output = Command::new(&cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(&workspace)
        .output()
        .expect("failed to spawn cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid cargo metadata");
    binary_path(&metadata, release).expect("cargo metadata target_directory")
}

fn binary_path(metadata: &Value, release: bool) -> Result<PathBuf, String> {
    let target = metadata["target_directory"]
        .as_str()
        .ok_or_else(|| "cargo metadata has no target_directory".to_string())?;
    Ok(Path::new(target)
        .join(if release { "release" } else { "debug" })
        .join(format!("falcon{}", std::env::consts::EXE_SUFFIX)))
}

/// Build the per-rule config passed to the falcon binary so that exactly this
/// rule fires (even non-recommended ones). Rules that ship a corpus
/// `config.json` (options/thresholds) reuse it verbatim; everything else gets a
/// minimal generated config written to `temp_path`.
pub(crate) fn docgen_config_for(
    corpus_root: &Path,
    meta: &RuleMeta,
    temp_path: &Path,
) -> Result<PathBuf, String> {
    let corpus_config = corpus_root.join(meta.name).join("config.json");
    if corpus_config.exists() {
        return Ok(corpus_config);
    }

    let mut group_map = Map::new();
    group_map.insert(meta.name.to_string(), json!("error"));

    let mut rules = Map::new();
    rules.insert("recommended".to_string(), json!(false));
    rules.insert(meta.group.as_str().to_string(), Value::Object(group_map));

    let cfg = if meta.is_cross_file() {
        json!({
            "linter": { "rules": { "recommended": false } },
            "cross-file": { "rules": rules },
        })
    } else {
        json!({
            "linter": { "rules": rules },
            "cross-file": { "rules": { "recommended": false } },
        })
    };

    let contents = serde_json::to_string_pretty(&cfg).map_err(|error| error.to_string())?;
    fs::write(temp_path, contents).map_err(|error| {
        format!(
            "{}: cannot write temp config {}: {error}",
            meta.name,
            temp_path.display()
        )
    })?;
    Ok(temp_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_max_errors_caps_json_stdout_without_changing_exit_status() {
        let binary = build_falcon(false);
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("falcon.json");
        fs::write(
            &config,
            r#"{"max-errors":1,"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}}"#,
        )
        .unwrap();
        for name in ["first.dart", "second.dart"] {
            fs::write(directory.path().join(name), "void broken( {").unwrap();
        }
        for (limit, expected_count) in [(None, 1), (Some("0"), 0), (Some("1"), 1)] {
            let mut command = Command::new(&binary);
            command
                .current_dir(directory.path())
                .args(["check", "--format", "json", "--exit-code", "7"])
                .arg("--config")
                .arg(&config);
            if let Some(limit) = limit {
                command.args(["--max-errors", limit]);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(7),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let diagnostics: Vec<DiagnosticJson> = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(diagnostics.len(), expected_count);
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.rule == "syntax-error")
            );
        }
    }

    #[test]
    fn diagnostic_json_keeps_unresolved_positions_absent() {
        let diagnostic: DiagnosticJson = serde_json::from_value(json!({
            "rule": "config",
            "message": "Cannot load configuration.",
            "file_path": "falcon.json"
        }))
        .unwrap();
        assert_eq!(diagnostic.line, None);
        assert_eq!(diagnostic.col, None);
        let error = diagnostic.resolved_position().unwrap_err();
        assert!(error.contains("config"), "{error}");
        assert!(error.contains("falcon.json"), "{error}");
    }

    #[test]
    fn diagnostic_json_requires_complete_nonzero_positions_for_source_examples() {
        for (line, col, expected) in [
            (Some(2), Some(3), Some((2, 3))),
            (Some(2), None, None),
            (None, Some(3), None),
            (Some(0), Some(0), None),
        ] {
            let diagnostic: DiagnosticJson = serde_json::from_value(json!({
                "rule": "example",
                "message": "Example.",
                "line": line,
                "col": col
            }))
            .unwrap();
            assert_eq!(diagnostic.resolved_position().ok(), expected);
        }
    }

    #[test]
    fn binary_path_uses_cargo_metadata_target_directory() {
        let metadata = json!({ "target_directory": "/custom/build output" });
        for (release, profile) in [(false, "debug"), (true, "release")] {
            assert_eq!(
                binary_path(&metadata, release).unwrap(),
                Path::new("/custom/build output")
                    .join(profile)
                    .join(format!("falcon{}", std::env::consts::EXE_SUFFIX))
            );
        }
        assert!(binary_path(&json!({}), false).is_err());
    }

    #[test]
    fn generated_config_write_failure_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let meta = &falcon_rules::meta::RULE_METADATA[0];
        let error = docgen_config_for(directory.path(), meta, directory.path()).unwrap_err();
        assert!(error.contains("cannot write temp config"), "{error}");
    }

    #[test]
    fn corpus_config_is_reused_without_writing_a_generated_config() {
        let directory = tempfile::tempdir().unwrap();
        let meta = &falcon_rules::meta::RULE_METADATA[0];
        let rule_dir = directory.path().join(meta.name);
        fs::create_dir(&rule_dir).unwrap();
        let config = rule_dir.join("config.json");
        fs::write(&config, "{}").unwrap();
        assert_eq!(
            docgen_config_for(directory.path(), meta, directory.path()).unwrap(),
            config
        );
    }
    #[test]
    fn staged_config_preserves_source_filters_override_order_and_options() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("source");
        fs::create_dir(&original).unwrap();
        for name in ["bad[1].dart", "good.dart", "excluded.dart"] {
            fs::write(original.join(name), "void f() {}").unwrap();
        }
        let config_path = original.join("config.json");
        let contents = json!({
            "files": { "includes": ["**/*.dart", "!**/excluded.dart"] },
            "overrides": [
                { "includes": ["**/*.dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": true }
                    }
                } } } },
                { "includes": ["**/bad[[]1].dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": false }
                    }
                } } } },
                { "includes": ["**/absent.dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": true }
                    }
                } } } }
            ]
        });
        let contents = serde_json::to_string(&contents).unwrap();
        fs::write(&config_path, &contents).unwrap();
        let staged = sdk_fixture::stage_project(&original);
        for target in [
            &original,
            &original.join("bad[1].dart"),
            &original.join("excluded.dart"),
        ] {
            let path = write_staged_config(&config_path, staged.path(), &original, target).unwrap();
            let config = falcon_rules::load_config(&path).unwrap().config;
            let bad =
                sdk_fixture::staged_file(staged.path(), &original, &original.join("bad[1].dart"));
            let excluded =
                sdk_fixture::staged_file(staged.path(), &original, &original.join("excluded.dart"));
            assert!(!config.files.includes.admits(&excluded.to_string_lossy()));
            if target.file_name().unwrap() != "excluded.dart" {
                assert!(config.files.includes.admits(&bad.to_string_lossy()));
                assert_eq!(config.overrides.len(), 2);
                assert_eq!(
                    config
                        .rule_options_data_for(
                            &bad.to_string_lossy(),
                            "performance",
                            "move-variable-closer-to-its-usage",
                        )
                        .map(falcon_config::RuleOptions::value),
                    Some(&json!({ "ignore-after-if": false })),
                );
                let good =
                    sdk_fixture::staged_file(staged.path(), &original, &original.join("good.dart"));
                assert_eq!(
                    config
                        .rule_options_data_for(
                            &good.to_string_lossy(),
                            "performance",
                            "move-variable-closer-to-its-usage",
                        )
                        .map(falcon_config::RuleOptions::value),
                    target
                        .is_dir()
                        .then_some(&json!({ "ignore-after-if": true })),
                );
            }
            assert_eq!(fs::read_to_string(&config_path).unwrap(), contents);
        }
    }

    #[cfg(unix)]
    #[test]
    fn runner_distinguishes_diagnostic_exit_codes_from_process_failures() {
        use std::os::unix::fs::PermissionsExt;
        for (status, stdout, expected_error) in [
            (2, "[]", Some("exit status: 2")),
            (1, "", Some("JSON parse error")),
            (1, "[]", None),
            (0, "[]", None),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let binary = directory.path().join("falcon");
            std::fs::write(
                &binary,
                format!(
                    "#!/bin/sh\nprintf '{stdout}'\nprintf 'fixture failure' >&2\nexit {status}\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
            let runner = super::FalconRunner {
                binary: binary.to_string_lossy().into_owned(),
                fallback_sdk: None,
            };
            let result = runner.run(&directory.path().join("clean.dart"), None);
            if let Some(expected) = expected_error {
                let error = result.unwrap_err();
                assert!(error.contains("fixture failure"), "{error}");
                assert!(error.contains(expected), "{error}");
            } else {
                assert!(result.unwrap().is_empty());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn bundled_fixtures_run_without_ambient_sdk_discovery() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let environment = directory.path().join("environment");
        let binary = directory.path().join("falcon");
        fs::write(
            &binary,
            format!(
                "#!/bin/sh\n/usr/bin/env > '{}'\nprintf '[]'\n",
                environment.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let child_environment = |fallback_sdk: Option<PathBuf>, file: &Path| {
            let runner = super::FalconRunner {
                binary: binary.to_string_lossy().into_owned(),
                fallback_sdk,
            };
            assert!(runner.run(file, None).unwrap().is_empty());
            fs::read_to_string(&environment).unwrap()
        };
        let sdk_variables = ["DART_SDK=", "FLUTTER_ROOT=", "PATH="];
        // Custom corpora keep the caller's environment, so PATH proves inheritance works.
        assert!(
            child_environment(None, &directory.path().join("clean.dart"))
                .lines()
                .any(|line| line.starts_with("PATH="))
        );
        let isolated = child_environment(
            Some(directory.path().join("sdk")),
            &directory.path().join("clean.dart"),
        );
        assert!(
            isolated.lines().all(|line| !sdk_variables
                .iter()
                .any(|variable| line.starts_with(variable))),
            "{isolated}"
        );
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/falcon_rules/tests/corpus/avoid-dynamic-calls/bad.dart");
        let staged = child_environment(Some(directory.path().join("sdk")), &fixture);
        assert!(staged.lines().any(|line| line.starts_with("DART_SDK=")));
        assert!(
            staged
                .lines()
                .all(|line| !line.starts_with("FLUTTER_ROOT=") && !line.starts_with("PATH=")),
            "{staged}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bare_binary_names_resolve_to_the_first_executable_match() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let [blocked, executable] = [("blocked", 0o600), ("bin", 0o700)].map(|(name, mode)| {
            let binary = directory.path().join(name).join("falcon");
            fs::create_dir(binary.parent().unwrap()).unwrap();
            fs::write(&binary, "").unwrap();
            fs::set_permissions(&binary, fs::Permissions::from_mode(mode)).unwrap();
            binary
        });
        let search_path = std::env::join_paths([
            directory.path().join("missing"),
            blocked.parent().unwrap().to_path_buf(),
            executable.parent().unwrap().to_path_buf(),
        ])
        .unwrap();
        let resolve = |binary: &str, search_path: Option<OsString>, current_directory: &Path| {
            super::resolve_binary(PathBuf::from(binary), search_path, current_directory)
        };
        let elsewhere = Path::new("/nonexistent-working-directory");
        assert_eq!(
            resolve("falcon", Some(search_path.clone()), elsewhere),
            executable
        );
        for unresolved in ["./falcon", "absent"] {
            assert_eq!(
                resolve(unresolved, Some(search_path.clone()), elsewhere),
                PathBuf::from(unresolved),
            );
        }
        assert_eq!(resolve("falcon", None, elsewhere), PathBuf::from("falcon"));
        // Empty and relative entries are anchored to the given directory, not left bare.
        let relative = std::env::join_paths([PathBuf::new(), PathBuf::from("bin")]).unwrap();
        assert_eq!(
            resolve("falcon", Some(relative.clone()), directory.path()),
            executable,
        );
        let empty = std::env::join_paths([PathBuf::new()]).unwrap();
        assert_eq!(
            resolve("falcon", Some(empty), executable.parent().unwrap()),
            executable,
        );
        assert_eq!(
            resolve("falcon", Some(relative), elsewhere),
            PathBuf::from("falcon"),
        );
    }

    #[test]
    fn fixture_sdk_is_always_used_for_the_bundled_corpus() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let bundled = workspace.join("crates/falcon_rules/tests/corpus");
        let fixture = workspace.join("crates/falcon_rules/tests/support/sdk");
        // Constructing the runner does not need or consult an ambient SDK locator.
        let runner = super::FalconRunner::new(
            std::path::PathBuf::from("unused-falcon"),
            workspace,
            &bundled,
        );
        assert_eq!(runner.fallback_sdk, Some(fixture));
        let custom = workspace.join("target/custom-corpus");
        assert_eq!(super::corpus_fixture_sdk(workspace, &custom), None);
    }

    #[test]
    fn fixture_sdk_only_applies_to_listed_source_files() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let sdk = workspace.join("crates/falcon_rules/tests/support/sdk");
        let runner = super::FalconRunner {
            binary: String::from("falcon"),
            fallback_sdk: Some(sdk.clone()),
        };
        let bundled = workspace.join("crates/falcon_rules/tests/corpus");
        assert_eq!(
            runner.sdk_for(&bundled.join("omit-local-variable-types/enum_values.dart")),
            Some(sdk.as_path()),
        );
        assert_eq!(
            runner.sdk_for(&bundled.join("omit-local-variable-types/good.dart")),
            None,
        );
        assert_eq!(runner.sdk_for(&bundled), Some(sdk.as_path()));
        let custom_runner = super::FalconRunner {
            binary: String::from("falcon"),
            fallback_sdk: super::corpus_fixture_sdk(
                workspace,
                &workspace.join("target/custom-corpus"),
            ),
        };
        assert_eq!(
            custom_runner.sdk_for(
                &workspace.join("target/custom-corpus/omit-local-variable-types/enum_values.dart"),
            ),
            None,
        );
    }

    #[test]
    fn fixture_sdk_applies_to_projects_containing_listed_sources() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let corpus = workspace.join("crates/falcon_rules/tests/corpus");
        let runner = super::FalconRunner::new(
            std::path::PathBuf::from("unused-falcon"),
            workspace,
            &corpus,
        );
        assert_eq!(
            runner.sdk_for(&corpus.join("public-member-api-docs/project")),
            runner.fallback_sdk.as_deref(),
        );
        assert!(runner.fallback_sdk.is_some());
        assert_eq!(
            runner.sdk_for(&corpus.join("always-use-package-imports/project")),
            None,
        );
    }

    #[test]
    fn fixture_inventory_ignores_blank_lines_and_surrounding_whitespace() {
        let fixtures = "\n \t\n omit-local-variable-types/enum_values.dart \n\n";
        assert!(super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("corpus/omit-local-variable-types/enum_values.dart"),
            fixtures,
        ));
        assert!(!super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("corpus/omit-local-variable-types/good.dart"),
            fixtures,
        ));
        assert!(!super::sdk_fixture::fixture_list_contains(
            std::path::Path::new("anything.dart"),
            "\n \t\n",
        ));
    }
}
