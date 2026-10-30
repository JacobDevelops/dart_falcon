use super::{WriteSnapshot, fix_report};
use std::fs;
use std::time::{Duration, SystemTime};

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
    assert!(error.contains("changed since it was read"));
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
    let parsed = falcon_dart_parser::try_parse_for_package(source, None).unwrap();
    let mut files = vec![falcon_analyze::ProjectFile {
        path: part.clone(),
        has_parse_errors: !parsed.errors.is_empty(),
        parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        }),
    }];
    let environment =
        falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([part.as_path()], |_| None);
    let external = super::FixExternals::load(&environment, &mut files);
    assert!(files.iter().any(|file| file.path == owner));
    assert!(files.iter().any(|file| file.path == part));
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
        let parsed = falcon_dart_parser::try_parse_for_package(before, None).unwrap();
        let mut file = falcon_analyze::ProjectFile {
            path: path.clone(),
            has_parse_errors: false,
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: before.into(),
                tree: parsed.tree,
            }),
        };
        let original = std::sync::Arc::clone(&file.parsed);
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
            assert!(std::sync::Arc::ptr_eq(&file.parsed, &original));
            assert_eq!(file.source(), before);
            assert_eq!(fs::read_to_string(&path).unwrap(), "// concurrent save\n");
        } else {
            result.unwrap();
            assert!(!std::sync::Arc::ptr_eq(&file.parsed, &original));
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
    let parsed = falcon_dart_parser::try_parse_for_package(before, None).unwrap();
    let mut file = falcon_analyze::ProjectFile {
        path: path.clone(),
        has_parse_errors: false,
        parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: before.into(),
            tree: parsed.tree,
        }),
    };
    let environment =
        falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    WriteSnapshot::apply_with_project_snapshot(None, &path, after, Some(&mut file), &environment)
        .unwrap();
    assert_eq!(file.source(), after);
    assert_eq!(fs::read_to_string(path).unwrap(), before);
}
