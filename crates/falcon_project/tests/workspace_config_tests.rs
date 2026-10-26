use falcon_analyze::AnalysisEnvironment;
use falcon_config::ResolvedSeverity;
use falcon_project::{
    ProjectAnalysis, analyze_sources, load_project_config, load_project_config_with_errors,
    walk_project,
};
use std::fs;
use std::path::{Path, PathBuf};

fn write(root: &Path, path: &str, source: &str) -> PathBuf {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, source).unwrap();
    path
}

fn workspace(root: &Path) {
    write(
        root,
        "pubspec.yaml",
        "name: workspace_root\nworkspace:\n  - packages/a\n  - packages/b\n",
    );
    for name in ["a", "b"] {
        write(
            root,
            &format!("packages/{name}/pubspec.yaml"),
            &format!("name: {name}\nresolution: workspace\n"),
        );
    }
}

#[test]
fn member_configuration_replaces_disabled_parent_and_controls_shared_analysis() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(
        root,
        "falcon.json",
        r#"{"linter":{"enabled":false},"files":{"includes":["!packages/**"]}}"#,
    );
    write(
        root,
        "packages/a/falcon.json",
        r#"{
        "files":{"includes":["lib/**","!lib/skipped.dart"]},
        "linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":{"level":"error","options":{"max-lines":1}}}}}
    }"#,
    );
    let admitted = write(
        root,
        "packages/a/lib/main.dart",
        "void main() {\n  return;\n}\n",
    );
    write(root, "packages/a/lib/skipped.dart", "void skipped() {}\n");
    write(root, "packages/a/test/test.dart", "void test() {}\n");
    write(root, "packages/b/lib/main.dart", "void main() {}\n");
    let loaded = load_project_config(None, &root.join("packages/a/lib")).unwrap();
    let config = loaded.config;
    assert_eq!(
        config.root_dir(),
        Some(root.canonicalize().unwrap().as_path())
    );
    assert!(config.is_rule_enabled_anywhere("complexity", "max-lines-for-file", false, &[]));
    assert_eq!(
        config.resolve_rule_for(
            &admitted.to_string_lossy(),
            "complexity",
            "max-lines-for-file",
            false,
            &[]
        ),
        Some(ResolvedSeverity::Error)
    );
    assert!(
        config
            .resolve_rule_for(
                &root.join("packages/b/lib/main.dart").to_string_lossy(),
                "complexity",
                "max-lines-for-file",
                false,
                &[]
            )
            .is_none()
    );
    let output = walk_project(&[root.to_path_buf()], &config.files.includes, true).unwrap();
    assert_eq!(output.files.len(), 1);
    assert_eq!(output.files[0].0, admitted);
    let analysis = ProjectAnalysis::new(&config);
    let mut environment =
        AnalysisEnvironment::discover_with_sdk_locator([admitted.as_path()], |_| None);
    let (diagnostics, _, _) = analyze_sources(
        &analysis,
        &output.files,
        &output.manifests,
        &config,
        &mut environment,
    );
    let diagnostics: Vec<_> = diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "max-lines-for-file")
        .collect();
    assert_eq!(diagnostics.len(), 1);
    let mut diagnostics = diagnostics;
    falcon_rules::apply_severities(&mut diagnostics, &config);
    assert_eq!(diagnostics[0].severity, falcon_diagnostics::Severity::Error);
}

#[test]
fn nearest_configuration_overrides_options_and_cross_file_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(
        root,
        "falcon.json",
        r#"{"cross-file":{"enabled":false},"linter":{"rules":{"recommended":false}}}"#,
    );
    write(
        root,
        "packages/falcon.json",
        r#"{"linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":{"level":"warn","options":{"max-lines":5}}}}}}"#,
    );
    write(
        root,
        "packages/a/falcon.json",
        r#"{
        "linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":{"level":"error","options":{"max-lines":2}}}}},
        "cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"error"}}},
        "overrides":[{"includes":["test/**"],"linter":{"rules":{"complexity":{"max-lines-for-file":{"level":"warn","options":{"max-lines":9}}}}}}]
    }"#,
    );
    let config = load_project_config(None, root).unwrap().config;
    for (path, expected) in [
        ("packages/a/lib/main.dart", 2),
        ("packages/a/test/main.dart", 9),
        ("packages/b/lib/main.dart", 5),
    ] {
        let path = root.join(path);
        let options = config
            .rule_options_data_for(&path.to_string_lossy(), "complexity", "max-lines-for-file")
            .unwrap();
        assert_eq!(options.value()["max-lines"], expected);
    }
    assert!(config.is_cross_file_rule_enabled_anywhere("correctness", "unused-files", false));
    assert_eq!(
        config.resolve_cross_file_rule_for(
            &root.join("packages/a/lib/main.dart").to_string_lossy(),
            "correctness",
            "unused-files",
            false
        ),
        Some(ResolvedSeverity::Error)
    );
    assert!(
        config
            .resolve_cross_file_rule_for(
                &root.join("main.dart").to_string_lossy(),
                "correctness",
                "unused-files",
                false
            )
            .is_none()
    );
}

#[test]
fn explicit_base_still_loads_members_and_cli_exclusions_remain_global() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    let base = write(root, "custom.json", r#"{"linter":{"enabled":false}}"#);
    write(
        root,
        "packages/a/falcon.json",
        r#"{"linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":"error"}}}}"#,
    );
    let file = write(root, "packages/a/lib/main.dart", "void main() {}\n");
    let unrelated = tempfile::tempdir().unwrap();
    let mut config = load_project_config(Some(&base), unrelated.path())
        .unwrap()
        .config;
    assert_eq!(
        config.resolve_rule_for(
            &file.to_string_lossy(),
            "complexity",
            "max-lines-for-file",
            false,
            &[]
        ),
        Some(ResolvedSeverity::Error)
    );
    config
        .files
        .includes
        .extend_excludes(&["packages/a/lib/**".into()]);
    assert!(!config.files.includes.admits(&file.to_string_lossy()));
    assert!(
        walk_project(&[root.to_path_buf()], &config.files.includes, false)
            .unwrap()
            .files
            .is_empty()
    );
}

#[test]
fn malformed_member_configuration_is_not_silently_ignored() {
    let temp = tempfile::tempdir().unwrap();
    workspace(temp.path());
    write(temp.path(), "falcon.json", "{}");
    write(
        temp.path(),
        "packages/a/falcon.json",
        r#"{"linter":{"enabld":false}}"#,
    );
    let error = load_project_config(None, temp.path()).unwrap_err();
    assert!(error.to_string().contains("enabld"));
    assert!(
        error.to_string().contains(
            &temp
                .path()
                .join("packages/a/falcon.json")
                .display()
                .to_string()
        )
    );
}

#[test]
fn tolerant_loading_retains_root_and_valid_members_with_qualified_errors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(
        root,
        "falcon.json",
        r#"{"max-errors":7,"linter":{"enabled":false}}"#,
    );
    let bad = write(
        root,
        "packages/a/falcon.json",
        r#"{"linter":{"enabld":false}}"#,
    );
    write(
        root,
        "packages/b/falcon.json",
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}}}"#,
    );
    let (loaded, errors) = load_project_config_with_errors(None, root).unwrap();
    assert_eq!(loaded.config.max_errors, Some(7));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].to_string().contains(&bad.display().to_string()));
    assert!(errors[0].to_string().contains("enabld"));
    assert!(!errors[0].1.is_empty());
    for (member, expected) in [("a", None), ("b", Some(ResolvedSeverity::Error))] {
        assert_eq!(
            loaded.config.resolve_rule_for(
                &root
                    .join(format!("packages/{member}/main.dart"))
                    .to_string_lossy(),
                "suspicious",
                "avoid-dynamic",
                false,
                &[],
            ),
            expected,
        );
    }
}

#[test]
fn tolerant_loading_still_rejects_invalid_base_configuration() {
    let temp = tempfile::tempdir().unwrap();
    workspace(temp.path());
    write(temp.path(), "falcon.json", "{");
    assert!(load_project_config_with_errors(None, temp.path()).is_err());
}

#[test]
fn selected_workspace_binds_discovered_home_config_to_input_root() {
    const CHILD: &str = "FALCON_SELECTED_WORKSPACE_HOME_CONFIG_TEST";
    if std::env::var_os(CHILD).is_none() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        workspace(&root);
        write(&root, "falcon.json", r#"{"linter":{"enabled":false}}"#);
        write(
            temp.path(),
            ".falcon.json",
            r#"{"files":{"includes":["packages/**"]}}"#,
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "selected_workspace_binds_discovered_home_config_to_input_root",
            ])
            .current_dir(temp.path())
            .env("HOME", temp.path())
            .env(CHILD, &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }
    let root = PathBuf::from(std::env::var_os(CHILD).unwrap())
        .canonicalize()
        .unwrap();
    let workspace = falcon_project::discover_workspace(&root).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let config = falcon_project::load_workspace_config(None, &cwd, Some(&workspace), None)
        .unwrap()
        .config;
    assert_eq!(config.root_dir(), Some(root.as_path()));
    assert!(
        config
            .files
            .includes
            .admits(&root.join("packages/a/lib/main.dart").to_string_lossy(),)
    );
    assert!(
        !config
            .files
            .includes
            .admits(&cwd.join("packages/a/lib/main.dart").to_string_lossy(),)
    );
    assert!(config.is_rule_enabled_anywhere("suspicious", "avoid-dynamic", true, &[]));
}

#[test]
fn nested_member_filters_keep_reenabled_descendants_walkable() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        "pubspec.yaml",
        "workspace:\n  - packages/app\n  - packages/app/example/nested\n",
    );
    for path in ["packages/app", "packages/app/example/nested"] {
        write(
            root,
            &format!("{path}/pubspec.yaml"),
            "name: member\nresolution: workspace\n",
        );
    }
    write(root, "falcon.json", "{}");
    write(
        root,
        "packages/app/falcon.json",
        r#"{"files":{"includes":["!example/**"]}}"#,
    );
    write(
        root,
        "packages/app/example/nested/falcon.json",
        r#"{"files":{"includes":["lib/**"]}}"#,
    );
    let file = write(
        root,
        "packages/app/example/nested/lib/main.dart",
        "void main() {}",
    );
    let config = load_project_config(None, root).unwrap().config;
    assert!(config.files.includes.admits(&file.to_string_lossy()));
    assert!(
        !config
            .files
            .includes
            .excludes_directory(&root.join("packages/app/example"))
    );
    let walked = walk_project(&[root.to_path_buf()], &config.files.includes, false).unwrap();
    assert_eq!(walked.files, [(file, "void main() {}".to_owned())]);
}

#[test]
fn member_defaults_and_domains_replace_parent_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(
        root,
        "falcon.json",
        r#"{
            "linter":{"domains":{"flutter":"none"},"rules":{"recommended":false}},
            "cross-file":{"enabled":false}
        }"#,
    );
    write(root, "packages/a/falcon.json", "{}");
    let config = load_project_config(None, root).unwrap().config;
    let child = root.join("packages/a/lib/main.dart");
    let fallback = root.join("packages/b/lib/main.dart");
    assert_eq!(
        config.resolve_rule_for(
            &child.to_string_lossy(),
            "suspicious",
            "avoid-print",
            true,
            &["flutter"],
        ),
        Some(ResolvedSeverity::Warn)
    );
    assert!(
        config
            .resolve_rule_for(
                &fallback.to_string_lossy(),
                "suspicious",
                "avoid-print",
                true,
                &["flutter"],
            )
            .is_none()
    );
    assert_eq!(
        config.resolve_cross_file_rule_for(
            &child.to_string_lossy(),
            "correctness",
            "unused-files",
            true,
        ),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn member_warnings_identify_their_configuration_file() {
    let temp = tempfile::tempdir().unwrap();
    workspace(temp.path());
    write(temp.path(), "falcon.json", "{}");
    let member = write(temp.path(), "packages/a/falcon.json", r#"{"max_errors":5}"#);
    let loaded = load_project_config(None, temp.path()).unwrap();
    assert!(loaded.warnings.iter().any(|warning| {
        warning.contains("max_errors") && warning.contains(&member.display().to_string())
    }));
}

#[test]
fn explicit_member_run_wide_settings_warn_once_even_at_their_defaults() {
    for (key, value) in [
        ("max-errors", "null"),
        ("max-errors", "5"),
        ("max_errors", "null"),
        ("max_errors", "5"),
        ("error-on-warnings", "false"),
        ("error-on-warnings", "true"),
        ("baseline", "null"),
        ("baseline", r#""member-baseline.json""#),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        workspace(root);
        write(
            root,
            "falcon.json",
            r#"{"max-errors":7,"error-on-warnings":true,"baseline":"root-baseline.json"}"#,
        );
        // Both members inherit this file, but it must only warn once.
        let member = write(
            root,
            "packages/falcon.json",
            &format!(r#"{{"{key}":{value}}}"#),
        );
        let loaded = load_project_config(None, root).unwrap();
        let warnings: Vec<_> = loaded
            .warnings
            .iter()
            .filter(|warning| warning.contains("ignores run-wide setting"))
            .collect();
        assert_eq!(warnings.len(), 1, "{key}: {warnings:?}");
        assert!(warnings[0].contains(&format!("`{key}`")));
        assert!(warnings[0].contains(&member.display().to_string()));
        assert_eq!(loaded.config.max_errors, Some(7));
        assert!(loaded.config.error_on_warnings);
        assert_eq!(
            loaded.config.baseline.as_deref(),
            Some(Path::new("root-baseline.json"))
        );
    }
}

#[test]
fn explicit_member_base_applies_run_wide_settings_without_ignored_setting_warnings() {
    for key in ["max-errors", "max_errors"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        workspace(root);
        write(root, "falcon.json", r#"{"max-errors":2}"#);
        write(
            root,
            "packages/a/falcon.json",
            &format!(
                r#"{{
                    "{key}":7,
                    "error-on-warnings":true,
                    "baseline":"member-baseline.json",
                    "files":{{"includes":["lib/**","["]}},
                    "linter":{{"rules":{{"recommended":false,"suspicious":{{"avoid-dynamic":"error"}}}}}}
                }}"#
            ),
        );
        // Resolve aliases before deciding whether a member is also the explicit base.
        let explicit = root.join("packages/a/../a/falcon.json");
        let loaded = load_project_config(Some(&explicit), root).unwrap();
        assert_eq!(loaded.config.max_errors, Some(7));
        assert!(loaded.config.error_on_warnings);
        assert_eq!(
            loaded.config.baseline.as_deref(),
            Some(Path::new("member-baseline.json"))
        );
        assert!(
            loaded
                .warnings
                .iter()
                .all(|warning| !warning.contains("ignores run-wide setting")),
            "{key}: {:?}",
            loaded.warnings
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("invalid path pattern"))
        );
        if key == "max_errors" {
            assert!(loaded.warnings.iter().any(|warning| {
                warning.contains("deprecated") && warning.contains("max_errors")
            }));
        }
        let file = root.join("packages/a/lib/main.dart");
        assert_eq!(
            loaded.config.resolve_rule_for(
                &file.to_string_lossy(),
                "suspicious",
                "avoid-dynamic",
                false,
                &[],
            ),
            Some(ResolvedSeverity::Error)
        );
        assert!(
            !loaded
                .config
                .files
                .includes
                .admits(&root.join("packages/a/test/main.dart").to_string_lossy())
        );
    }
}

#[test]
fn absent_member_run_wide_settings_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(
        root,
        "falcon.json",
        r#"{"max-errors":7,"error-on-warnings":true,"baseline":"root-baseline.json"}"#,
    );
    write(root, "packages/falcon.json", "{}");
    let loaded = load_project_config(None, root).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn global_exclusions_escape_workspace_directory_metacharacters() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace[v2]");
    workspace(&root);
    write(&root, "falcon.json", "{}");
    write(&root, "packages/a/falcon.json", "{}");
    let file = write(&root, "packages/a/lib/gen/main.dart", "void main() {}");
    let mut config = load_project_config(None, &root).unwrap().config;
    config.files.includes.extend_excludes(&["**/gen/**".into()]);
    assert!(!config.files.includes.admits(&file.to_string_lossy()));
    assert!(
        walk_project(&[root], &config.files.includes, false)
            .unwrap()
            .files
            .is_empty()
    );
}

#[test]
fn local_nonmember_configuration_is_preserved_inside_a_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    workspace(root);
    write(root, "falcon.json", r#"{"linter":{"enabled":false}}"#);
    write(root, "tools/scripts/pubspec.yaml", "name: scripts\n");
    write(
        root,
        "tools/scripts/falcon.json",
        r#"{"linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":"error"}}}}"#,
    );
    let file = write(root, "tools/scripts/main.dart", "void main() {}");
    let config = load_project_config(None, &root.join("tools/scripts"))
        .unwrap()
        .config;
    assert_eq!(
        config.resolve_rule_for(
            &file.to_string_lossy(),
            "complexity",
            "max-lines-for-file",
            false,
            &[],
        ),
        Some(ResolvedSeverity::Error)
    );
}

#[test]
fn ignored_melos_package_retains_its_local_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        "melos.yaml",
        "packages:\n  - packages/**\nignore:\n  - packages/app/example\n",
    );
    write(root, "packages/app/pubspec.yaml", "name: app\n");
    write(root, "packages/app/example/pubspec.yaml", "name: example\n");
    write(root, "falcon.json", r#"{"linter":{"enabled":false}}"#);
    write(
        root,
        "packages/app/falcon.json",
        r#"{"linter":{"enabled":false}}"#,
    );
    write(
        root,
        "packages/app/example/falcon.json",
        r#"{"linter":{"rules":{"recommended":false,"complexity":{"max-lines-for-file":"error"}}}}"#,
    );
    let file = write(root, "packages/app/example/main.dart", "void main() {}");
    let config = load_project_config(None, &root.join("packages/app/example"))
        .unwrap()
        .config;
    assert_eq!(
        config.resolve_rule_for(
            &file.to_string_lossy(),
            "complexity",
            "max-lines-for-file",
            false,
            &[],
        ),
        Some(ResolvedSeverity::Error)
    );
}

#[test]
fn explicit_home_configuration_binds_to_start_directory_not_process_directory() {
    const CHILD: &str = "FALCON_TEST_PROJECT_EXPLICIT_HOME_ROOT";
    if std::env::var_os(CHILD).is_none() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            ".falcon.json",
            r#"{"files":{"includes":["lib/**"]}}"#,
        );
        let project = temp.path().join("project");
        fs::create_dir_all(project.join("other")).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "explicit_home_configuration_binds_to_start_directory_not_process_directory",
            ])
            .env(CHILD, "1")
            .env("HOME", temp.path())
            .current_dir(project)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }
    let process_directory = std::env::current_dir().unwrap();
    let start = process_directory.join("other");
    let home_config = process_directory.parent().unwrap().join(".falcon.json");
    let config = load_project_config(Some(&home_config), &start)
        .unwrap()
        .config;
    assert_eq!(
        config.root_dir(),
        Some(start.canonicalize().unwrap().as_path())
    );
    assert!(
        config
            .files
            .includes
            .admits(&start.join("lib/main.dart").to_string_lossy())
    );
    assert!(
        !config
            .files
            .includes
            .admits(&process_directory.join("lib/main.dart").to_string_lossy())
    );
}
