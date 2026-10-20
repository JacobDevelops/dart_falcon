use crate::workspace_root;
use falcon_rules::meta::RuleMeta;
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, serde::Deserialize)]
pub(crate) struct DiagnosticJson {
    pub(crate) rule: String,
    pub(crate) message: String,
    pub(crate) line: usize,
    pub(crate) col: usize,
    #[serde(default)]
    pub(crate) file_path: String,
}

#[path = "../../crates/falcon_rules/tests/support/sdk_fixture.rs"]
mod sdk_fixture;

/// Bundled listed corpus fixtures always use parsed dependency sources,
/// independently of the developer machine's installed SDK.
pub(crate) struct FalconRunner {
    pub(crate) binary: String,
    pub(crate) fallback_sdk: Option<PathBuf>,
}

impl FalconRunner {
    pub(crate) fn new(binary: PathBuf, workspace: &Path, corpus: &Path) -> Self {
        let fallback_sdk = corpus_fixture_sdk(workspace, corpus);
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
        let mut cmd = Command::new(&self.binary);
        cmd.args(["check", "--format", "json"]);
        cmd.arg(&analyzed_file);
        if let Some(config_path) = config {
            cmd.args(["--config", config_path.to_str().unwrap_or("")]);
        }
        if let Some((project, _)) = &staged {
            cmd.env(
                "DART_SDK",
                project.path().join("flutter/bin/cache/dart-sdk"),
            );
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

fn corpus_fixture_sdk(workspace: &Path, corpus: &Path) -> Option<PathBuf> {
    let bundled = workspace.join("crates/falcon_rules/tests/corpus");
    (falcon_analyze::canonical_or_lexical(corpus) == falcon_analyze::canonical_or_lexical(&bundled))
        .then(|| workspace.join("crates/falcon_rules/tests/support/sdk"))
}

pub(crate) fn recursive_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("cannot walk {}: {error}", root.display()))
        })
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
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
    command.args(["build", "--locked", "--bin", "falcon"]);
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
