use super::{WriteSnapshot, fix_report};
use std::fs;
use std::time::{Duration, SystemTime};

#[test]
fn partial_write_file_reports_do_not_repeat_the_summary_lists() {
    let partial = super::PartialFixWrite {
        written: vec!["first.dart".into()],
        not_written: vec!["second.dart".into(), "third.dart".into()],
        cause: "disk conflict".into(),
    };
    let message = super::partial_fix_message(&partial);
    assert!(message.contains("not written"));
    assert!(!message.contains("first.dart"), "{message}");
    assert!(partial.to_string().contains("first.dart"));
}

#[test]
fn malformed_workspace_root_does_not_block_source_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.dart");
    fs::write(&source, "void main() {}").unwrap();
    fs::write(dir.path().join("pubspec.yaml"), "workspace: [").unwrap();
    assert!(falcon_project::discover_workspace(dir.path()).is_none());
    let output = super::collect_check_with_sdk_locator(
        &super::CheckOptions {
            paths: vec![source],
            ..Default::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(output.total_files, 1);
}

#[test]
fn relative_missing_nested_input_uses_unmatched_policy() {
    let output = super::collect_check_with_sdk_locator(
        &super::CheckOptions {
            paths: vec!["nonexistent-lifecycle-input/nested/main.dart".into()],
            no_errors_on_unmatched: true,
            ..Default::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(output.exit_code, 0);
}

#[cfg(unix)]
#[test]
fn unreadable_excluded_explicit_input_warns_without_blocking_linting() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf, std::fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::write(private.join("main.dart"), "void main() {}").unwrap();
    let config = directory.path().join("falcon.json");
    fs::write(&config, r#"{"files":{"includes":["**","!private"]}}"#).unwrap();
    let _restore = Restore(private.clone(), private.metadata().unwrap().permissions());
    fs::set_permissions(&private, fs::Permissions::from_mode(0o0)).unwrap();
    if std::process::Command::new("id")
        .arg("-u")
        .output()
        .unwrap()
        .stdout
        == b"0\n"
        || fs::read_dir(&private).is_ok()
    {
        return;
    }
    let output = super::collect_check_with_sdk_locator(
        &super::CheckOptions {
            paths: vec![private.join("main.dart")],
            config_path: Some(config),
            ..Default::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.total_files, 0);
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule == super::UNMATCHED_PATH_DIAGNOSTIC_ID)
    );
}

#[test]
fn capture_conflict_aborts_all_staged_fixes() {
    let directory = tempfile::tempdir().unwrap();
    let before = "class Example {} void main() { new Example(); }\n";
    let mut files: Vec<_> = ["first.dart", "second.dart"]
        .into_iter()
        .map(|name| {
            let path = directory.path().join(name);
            fs::write(&path, before).unwrap();
            (path, before.to_owned())
        })
        .collect();
    fs::write(&files[1].0, "// independent save\n").unwrap();
    let config: falcon_config::FalconConfig = serde_json::from_str(
        r#"{"linter":{"rules":{"recommended":false,"style":{"unnecessary-new":"error"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let analysis = falcon_project::ProjectAnalysis::new(&config);
    let mut environment = falcon_analyze::AnalysisEnvironment::with_sdk_locator(|_| None);
    let mut diff = String::new();
    let mut reports = Vec::new();
    super::apply_selected_fixes(
        &super::CheckOptions {
            write: true,
            ..Default::default()
        },
        &analysis,
        &config,
        &mut environment,
        (&mut files, &[]),
        directory.path(),
        (&mut diff, &mut reports),
    )
    .unwrap();
    assert!(
        reports
            .iter()
            .any(|report| report.message.contains("changed since it was read"))
    );
    assert_eq!(fs::read_to_string(&files[0].0).unwrap(), before);
    assert_eq!(
        fs::read_to_string(&files[1].0).unwrap(),
        "// independent save\n"
    );
}

#[test]
fn changed_length_skips_write_and_reports_without_leaving_a_tempfile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "original").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::write(&path, "user saved a longer source").unwrap();
    let error = snapshot.write(&path, "fixed").unwrap_err();
    let report = fix_report(&path, &error);
    assert_eq!(report.rule, "autofix");
    assert!(report.message.contains("mtime or length"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "user saved a longer source"
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn changed_mtime_with_same_length_skips_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "original").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::write(&path, "new text").unwrap();
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_ne!(snapshot.modified, modified);
    let error = snapshot.write(&path, "fixed").unwrap_err();
    assert!(error.contains("mtime or length"));
    assert_eq!(fs::read_to_string(path).unwrap(), "new text");
}

#[test]
fn changes_between_source_read_and_snapshot_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "user edit").unwrap();
    let error = WriteSnapshot::capture(&path, "old source").err().unwrap();
    assert!(matches!(error, super::CaptureFailure::Conflict(_)));
    assert_eq!(fs::read_to_string(path).unwrap(), "user edit");
}

#[cfg(unix)]
#[test]
fn replacing_source_with_symlink_after_snapshot_skips_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    let target = dir.path().join("target.dart");
    fs::write(&path, "original").unwrap();
    fs::write(&target, "external").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let error = snapshot.write(&path, "fixed").unwrap_err();
    assert!(error.contains("symlink"));
    assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), "external");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn resolving_write_loads_one_fix_external_cache_independent_of_file_count() {
    for count in [1, 8] {
        for dry_run in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let config = dir.path().join("falcon.json");
            fs::write(
                &config,
                r#"{
                    "linter": { "rules": {
                        "recommended": false,
                        "style": { "unnecessary-new": "error" },
                        "complexity": { "unnecessary-overrides": "error" }
                    }},
                    "cross-file": { "enabled": false }
                }"#,
            )
            .unwrap();
            // Keep a dependency outside the selected inputs so the cache has
            // actual external declarations, not just an empty closure.
            fs::write(dir.path().join("dependency.dart"), "class Example {}\n").unwrap();
            let sources = dir.path().join("lib");
            fs::create_dir(&sources).unwrap();
            let before =
                "import '../dependency.dart';\nvoid main() { new Example(); new Example(); }\n";
            for index in 0..count {
                fs::write(sources.join(format!("file{index}.dart")), before).unwrap();
            }
            super::FIX_EXTERNAL_LOADS.with(|loads| loads.set(0));
            let output = super::collect_check_with_sdk_locator(
                &super::CheckOptions {
                    paths: vec![sources.clone()],
                    config_path: Some(config),
                    write: true,
                    dry_run,
                    ..super::CheckOptions::default()
                },
                |_| None,
            )
            .unwrap();
            assert_eq!(output.total_files, count);
            assert_eq!(
                output.exit_code,
                i32::from(dry_run),
                "{:?}",
                output.diagnostics
            );
            assert_eq!(super::FIX_EXTERNAL_LOADS.with(std::cell::Cell::get), 1);
            for index in 0..count {
                assert_eq!(
                    fs::read_to_string(sources.join(format!("file{index}.dart"))).unwrap(),
                    if dry_run {
                        before.to_owned()
                    } else {
                        before.replace("new ", "")
                    }
                );
            }
        }
    }
}

#[test]
fn resolving_write_without_fixable_files_does_not_load_fix_externals() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("falcon.json");
    fs::write(
        &config,
        r#"{
            "linter": { "rules": {
                "recommended": false,
                "complexity": { "unnecessary-overrides": "error" }
            }},
            "cross-file": { "enabled": false }
        }"#,
    )
    .unwrap();
    fs::write(dir.path().join("main.dart"), "class Example {}\n").unwrap();
    super::FIX_EXTERNAL_LOADS.with(|loads| loads.set(0));
    let output = super::collect_check_with_sdk_locator(
        &super::CheckOptions {
            paths: vec![dir.path().to_path_buf()],
            config_path: Some(config),
            write: true,
            ..super::CheckOptions::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(output.exit_code, 0);
    assert_eq!(super::FIX_EXTERNAL_LOADS.with(std::cell::Cell::get), 0);
}

#[test]
fn structural_write_does_not_load_fix_externals() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("falcon.json");
    fs::write(
        &config,
        r#"{
            "linter": { "rules": {
                "recommended": false,
                "style": { "unnecessary-new": "error" }
            }},
            "cross-file": { "enabled": false }
        }"#,
    )
    .unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "class Example {} void main() { new Example(); }\n").unwrap();
    super::FIX_EXTERNAL_LOADS.with(|loads| loads.set(0));
    let output = super::collect_check_with_sdk_locator(
        &super::CheckOptions {
            paths: vec![path.clone()],
            config_path: Some(config),
            write: true,
            ..super::CheckOptions::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(output.exit_code, 0);
    assert_eq!(super::FIX_EXTERNAL_LOADS.with(std::cell::Cell::get), 0);
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "class Example {} void main() { Example(); }\n"
    );
}

#[test]
fn shared_fix_externals_retain_unopened_selected_part_companions() {
    let dir = tempfile::tempdir().unwrap();
    let owner = dir.path().join("owner.dart");
    let part = dir.path().join("part.dart");
    fs::write(&owner, "part 'part.dart'; class Base {}\n").unwrap();
    let source = "part of 'owner.dart'; class Child extends Base {}\n";
    fs::write(&part, source).unwrap();
    let mut files = vec![
        falcon_analyze::ProjectFile::new(
            part.clone(),
            std::sync::Arc::new(
                falcon_analyze::ParsedFile::try_parse_for_package(source, None)
                    .unwrap()
                    .0,
            ),
        )
        .expect("selected part fixture parses successfully"),
    ];
    let environment =
        falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([part.as_path()], |_| None);
    let external = super::FixExternals::load(&environment, &mut files).unwrap();
    assert!(files.iter().any(|file| file.path() == owner));
    assert!(files.iter().any(|file| file.path() == part));
    assert!(external.libraries.project_companions().next().is_some());
}

#[test]
fn saved_writes_refresh_project_snapshots_but_rejected_writes_do_not() {
    for concurrent_save in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.dart");
        let before = "class Example {} void main() { new Example(); }\n";
        let after = "class Example {} void main() { Example(); }\n";
        fs::write(&path, before).unwrap();
        let mut file = falcon_analyze::ProjectFile::new(
            path.clone(),
            std::sync::Arc::new(
                falcon_analyze::ParsedFile::try_parse_for_package(before, None)
                    .unwrap()
                    .0,
            ),
        )
        .expect("saved write fixture parses successfully");
        let original = std::sync::Arc::clone(file.parsed());
        let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            [path.as_path()],
            |_| None,
        );
        let snapshot = WriteSnapshot::capture(&path, before).unwrap();
        if concurrent_save {
            fs::write(&path, "// concurrent save\n").unwrap();
        }
        let result = WriteSnapshot::apply_with_project_snapshot(
            Some(snapshot),
            &path,
            after,
            Some(&mut file),
            &environment,
        );
        if concurrent_save {
            assert!(result.is_err());
            assert!(std::sync::Arc::ptr_eq(file.parsed(), &original));
            assert_eq!(file.source(), before);
            assert_eq!(fs::read_to_string(&path).unwrap(), "// concurrent save\n");
        } else {
            result.unwrap();
            assert!(!std::sync::Arc::ptr_eq(file.parsed(), &original));
            assert_eq!(file.source(), after);
            assert_eq!(fs::read_to_string(&path).unwrap(), after);
        }
    }
}

#[test]
fn preview_refreshes_project_snapshot_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    let before = "class Example {} void main() { new Example(); }\n";
    let after = "class Example {} void main() { Example(); }\n";
    fs::write(&path, before).unwrap();
    let mut file = falcon_analyze::ProjectFile::new(
        path.clone(),
        std::sync::Arc::new(
            falcon_analyze::ParsedFile::try_parse_for_package(before, None)
                .unwrap()
                .0,
        ),
    )
    .expect("preview fixture parses successfully");
    let environment =
        falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    WriteSnapshot::apply_with_project_snapshot(None, &path, after, Some(&mut file), &environment)
        .unwrap();
    assert_eq!(file.source(), after);
    assert_eq!(fs::read_to_string(path).unwrap(), before);
}

#[test]
fn worker_failure_forces_nonzero_and_skips_cross_file_until_recovery() {
    use falcon_analyze::{AnalyzeContext, CrossFileRule, Rule, RulePreparation, RuleRegistry};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::{Diagnostic, Severity, Span};

    struct Panics;
    impl Rule for Panics {
        fn prepare<'a>(
            &self,
            _: falcon_syntax::cst::Program<'a>,
            _: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            panic!("file worker regression");
        }
    }
    struct CurrentSource;
    impl CrossFileRule for CurrentSource {
        fn prepare_project(
            &self,
            _: &falcon_analyze::AnalysisEnvironment,
            files: &[falcon_analyze::ProjectFile],
            _: &FalconConfig,
            _: Option<&falcon_analyze::SemanticIndexes>,
        ) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                "current-source",
                Severity::Warning,
                files[0].source(),
                files[0].path().to_string_lossy(),
                Span { start: 0, end: 0 },
            )]
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.dart");
    let environment =
        falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    let config = FalconConfig::default();
    let mut rules = RuleRegistry::new();
    rules.register("panics", Box::new(Panics));
    let mut cross = falcon_analyze::CrossFileRuleRegistry::new();
    cross.register("current-source", Box::new(CurrentSource));
    let options = super::CheckOptions {
        error_exit_code: 0,
        ..Default::default()
    };
    let inputs = vec![(path.clone(), "class Before {}".to_owned())];
    let (mut diagnostics, files, _) = falcon_analyze::analyze_sequential_collecting_resolving(
        &rules,
        &inputs,
        &config,
        true,
        false,
        falcon_analyze::Workspace::default(),
    )
    .unwrap();
    let failed = falcon_project::complete_analysis(
        &environment,
        &cross,
        &files,
        &config,
        None,
        &mut diagnostics,
    );
    assert!(failed);
    assert_eq!(
        super::check_exit_code(&options, false, &diagnostics, failed),
        1
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].file_path, path.to_string_lossy());
    assert_eq!(
        diagnostics[0].rule,
        falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
    );
    assert!(diagnostics[0].message.contains("file worker regression"));

    let inputs = vec![(path, "class After {}".to_owned())];
    let (mut diagnostics, files, _) = falcon_analyze::analyze_sequential_collecting_resolving(
        &RuleRegistry::new(),
        &inputs,
        &config,
        true,
        false,
        falcon_analyze::Workspace::default(),
    )
    .unwrap();
    let failed = falcon_project::complete_analysis(
        &environment,
        &cross,
        &files,
        &config,
        None,
        &mut diagnostics,
    );
    assert!(!failed);
    assert_eq!(
        super::check_exit_code(&options, false, &diagnostics, failed),
        0
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "current-source");
    assert_eq!(diagnostics[0].message, "class After {}");
}

#[test]
fn staged_write_failure_reports_committed_and_unwritten_paths() {
    let directory = tempfile::tempdir().unwrap();
    let mut files: Vec<_> = ["first.dart", "second.dart", "third.dart"]
        .into_iter()
        .map(|name| {
            let path = directory.path().join(name);
            fs::write(&path, "original").unwrap();
            (path, "original".to_owned())
        })
        .collect();
    let staged = files
        .iter()
        .enumerate()
        .map(|(index, (path, source))| super::StagedFix {
            index,
            source: "fixed".to_owned(),
            snapshot: Some(WriteSnapshot::capture(path, source).unwrap()),
        })
        .collect();
    fs::write(&files[1].0, "independent save").unwrap();
    let failure = super::commit_staged_fixes(&mut files, staged).unwrap_err();
    assert_eq!(failure.written, [files[0].0.clone()]);
    assert_eq!(
        failure.not_written,
        [files[1].0.clone(), files[2].0.clone()]
    );
    assert_eq!(fs::read_to_string(&files[0].0).unwrap(), "fixed");
    assert_eq!(fs::read_to_string(&files[1].0).unwrap(), "independent save");
    assert_eq!(fs::read_to_string(&files[2].0).unwrap(), "original");
    assert_eq!(files[0].1, "fixed");
    assert_eq!(files[2].1, "original");
}
