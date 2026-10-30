use super::*;

fn workspace(dynamic: bool) -> (tempfile::TempDir, LspState, String) {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let main = directory.path().join("main.dart");
    std::fs::write(&main, "void main() {}").unwrap();
    let uri = format!("file://{}", main.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.set_dynamic_file_watching(dynamic);
    state.resolve_semantics = true;
    (directory, state, uri)
}

#[test]
fn snapshot_rebuild_revalidates_disk_parses_without_a_watcher_event() {
    let (directory, mut state, uri) = workspace(true);
    let closed = directory.path().join("closed.dart");
    std::fs::write(&closed, "class Before {}").unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    let reads = state.disk_read_count;
    let parses = state.disk_parse_count;
    let cached = Arc::clone(&state.disk_files[&closed].parsed);
    state.change(&uri, "void renamed() {}".into(), Some(2));
    state.analyze(&uri);
    assert_eq!(state.disk_read_count, reads);
    assert_eq!(state.disk_parse_count, parses);
    assert!(Arc::ptr_eq(&cached, &state.disk_files[&closed].parsed));

    let modified = std::fs::metadata(&closed).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(&closed)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    state.change(&uri, "void afterTouch() {}".into(), Some(3));
    state.analyze(&uri);
    assert_eq!(state.disk_read_count, reads + 1);
    assert_eq!(state.disk_parse_count, parses);
    assert!(Arc::ptr_eq(&cached, &state.disk_files[&closed].parsed));

    std::fs::write(&closed, "class ChangedWithoutAnEvent {}").unwrap();
    state.change(&uri, "void anotherName() {}".into(), Some(4));
    state.analyze(&uri);
    assert_eq!(state.disk_read_count, reads + 2);
    assert_eq!(state.disk_parse_count, parses + 1);
    assert_eq!(
        state.disk_files[&closed].parsed.source.as_ref(),
        "class ChangedWithoutAnEvent {}"
    );
}

#[test]
fn directory_deletion_event_removes_all_closed_sources() {
    let (directory, mut state, uri) = workspace(true);
    let nested = directory.path().join("closed");
    std::fs::create_dir(&nested).unwrap();
    for name in ["one.dart", "two.dart"] {
        std::fs::write(nested.join(name), "class Closed {}").unwrap();
    }
    state.open(&uri, "void main() {}".into(), Some(1));
    assert_eq!(state.disk_files.len(), 2);
    std::fs::remove_dir_all(&nested).unwrap();
    state.watched_files_changed(std::slice::from_ref(&nested));
    assert!(
        state
            .disk_files
            .keys()
            .all(|path| !path.starts_with(&nested))
    );
    assert!(
        state
            .semantic_snapshot
            .as_ref()
            .unwrap()
            .files
            .iter()
            .all(|file| !file.path.starts_with(&nested))
    );
}

#[test]
fn fallback_boundary_refreshes_closed_edits_deletions_and_new_directories() {
    let (directory, mut state, uri) = workspace(false);
    let closed = directory.path().join("closed.dart");
    std::fs::write(&closed, "class Before {}").unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    std::fs::write(&closed, "class AfterLongerName {}").unwrap();
    let updates = state.revalidate_disk();
    assert!(updates.iter().any(|(updated, _)| updated == &uri));
    assert_eq!(
        state.disk_files[&closed].parsed.source.as_ref(),
        "class AfterLongerName {}"
    );

    std::fs::remove_file(&closed).unwrap();
    let nested = directory.path().join("new_directory");
    std::fs::create_dir(&nested).unwrap();
    let added = nested.join("added.dart");
    std::fs::write(&added, "class Added {}").unwrap();
    // Make directory-stamp changes deterministic even on coarse filesystems.
    for path in [directory.path(), nested.as_path()] {
        let modified = std::fs::metadata(path).unwrap().modified().unwrap();
        std::fs::File::open(path)
            .unwrap()
            .set_modified(modified + std::time::Duration::from_secs(2))
            .unwrap();
    }
    state.revalidate_disk();
    assert!(!state.disk_files.contains_key(&closed));
    assert!(state.disk_files.contains_key(&added));
    std::fs::remove_dir_all(&nested).unwrap();
    state.revalidate_disk();
    assert!(!state.disk_files.contains_key(&added));
}

#[test]
fn fallback_keeps_closed_and_companion_edits_pending_across_analysis_boundaries() {
    for companion in [false, true] {
        for rebuild in [false, true] {
            let (directory, mut state, uri) = workspace(false);
            std::fs::write(
                directory.path().join("falcon.json"),
                r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
            )
            .unwrap();
            state.reload_config();
            let outside = tempfile::tempdir().unwrap();
            let (closed, directive, prefix) = if companion {
                let outside_name = outside.path().file_name().unwrap().to_str().unwrap();
                let workspace_name = directory.path().file_name().unwrap().to_str().unwrap();
                (
                    outside.path().join("companion.dart"),
                    format!("part '../{outside_name}/companion.dart';"),
                    format!("part of '../{workspace_name}/main.dart'; "),
                )
            } else {
                (
                    directory.path().join("closed.dart"),
                    "import 'closed.dart';".to_owned(),
                    String::new(),
                )
            };
            let before = format!("{prefix}void api() {{}}");
            let after = format!("{prefix}int api() => 1;");
            std::fs::write(&closed, &before).unwrap();
            let initial = state.open(
                &uri,
                format!("{directive} void main() {{ api(); }}"),
                Some(1),
            );
            assert!(
                initial
                    .iter()
                    .all(|diagnostic| diagnostic.rule != "avoid-ignoring-return-values")
            );
            state.cross_file_pass();
            let stamp = state.fallback_stamps[&closed].clone();
            assert!(stamp.is_some());
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert_eq!(snapshot.files[snapshot.by_path[&closed]].source(), before);

            std::fs::write(&closed, &after).unwrap();
            if rebuild {
                state.change(
                    &uri,
                    format!("{directive} void renamed() {{ api(); }}"),
                    Some(2),
                );
                state.ensure_semantic_snapshot();
            } else {
                state.cross_file_pass();
                let snapshot = state.semantic_snapshot.as_ref().unwrap();
                assert_eq!(snapshot.files[snapshot.by_path[&closed]].source(), before);
            }
            assert_eq!(state.fallback_stamps[&closed], stamp);
            let published = state.revalidate_disk();
            assert!(
                published.iter().any(|(published_uri, diagnostics)| {
                    published_uri == &uri
                        && diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
                }),
                "companion={companion}, rebuild={rebuild}: {published:?}"
            );
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert_eq!(snapshot.files[snapshot.by_path[&closed]].source(), after);
            assert_ne!(state.fallback_stamps[&closed], stamp);
            assert!(state.revalidate_disk().is_empty());
        }
    }
}

#[test]
fn text_changes_and_normal_ensure_never_scan_fallback_stamps() {
    for dynamic in [false, true] {
        let (directory, mut state, uri) = workspace(dynamic);
        let closed = directory.path().join("closed.dart");
        std::fs::write(&closed, "class Before {}").unwrap();
        state.open(&uri, "void main() {}".into(), Some(1));
        let reads = state.disk_read_count;
        let walks = state.workspace_walk_count;
        std::fs::write(&closed, "class ChangedOnDisk {}").unwrap();
        for version in 2..8 {
            state.change(
                &uri,
                format!("void main() {{ print({version}); }}"),
                Some(version),
            );
            state.ensure_semantic_snapshot();
        }
        assert_eq!(state.fallback_scan_count, 0);
        assert_eq!(state.disk_read_count, reads);
        assert_eq!(state.workspace_walk_count, walks);
        assert_eq!(
            state.disk_files[&closed].parsed.source.as_ref(),
            "class Before {}"
        );
        if dynamic {
            assert_eq!(state.project_discovery_watches, []);
            assert!(state.fallback_stamps.is_empty());
            assert!(state.revalidate_disk().is_empty());
            assert_eq!(state.fallback_scan_count, 0);
        }
    }
}

#[test]
fn unrelated_asset_events_leave_project_caches_intact() {
    let (directory, mut state, uri) = workspace(true);
    state.open(&uri, "void main() {}".into(), Some(1));
    let builds = state.semantic_snapshot_build_count;
    let walks = state.workspace_walk_count;
    let asset = directory.path().join("image.png");
    std::fs::write(&asset, "not a source").unwrap();
    assert!(state.watched_files_changed(&[asset]).is_empty());
    assert_eq!(state.semantic_snapshot_build_count, builds);
    assert_eq!(state.workspace_walk_count, walks);
}

#[test]
fn unsaved_document_identity_is_not_probed_during_body_edits() {
    let (directory, mut state, _) = workspace(true);
    let path = directory.path().join("unsaved.dart");
    let uri = format!("file://{}", path.display());
    state.open(&uri, "void unsaved() {}".into(), Some(1));
    assert!(state.documents[&uri].unresolved_path);
    std::fs::write(&path, "void unsaved() {}").unwrap();
    for version in 2..5 {
        state.change(
            &uri,
            format!("void unsaved() {{ print({version}); }}"),
            Some(version),
        );
        state.analyze(&uri);
        state.cross_file_pass();
        state.ensure_semantic_snapshot();
        assert!(state.documents[&uri].unresolved_path);
    }
    state.watched_files_changed(&[path]);
    assert!(!state.documents[&uri].unresolved_path);
}

#[test]
fn rejected_dynamic_watching_discovers_sources_at_idle() {
    let (directory, mut state, uri) = workspace(true);
    let closed = directory.path().join("closed.dart");
    std::fs::write(&closed, "class Before {}").unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    assert_eq!(state.project_discovery_watches, []);
    state.set_dynamic_file_watching(false);
    std::fs::write(&closed, "class ChangedAfterRejection {}").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert_eq!(
        state.disk_files[&closed].parsed.source.as_ref(),
        "class ChangedAfterRejection {}"
    );
    assert_ne!(state.project_discovery_watches, []);
    let nested = directory.path().join("new_directory");
    std::fs::create_dir(&nested).unwrap();
    let added = nested.join("added.dart");
    std::fs::write(&added, "class Added {}").unwrap();
    let modified = std::fs::metadata(directory.path())
        .unwrap()
        .modified()
        .unwrap();
    std::fs::File::open(directory.path())
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    state.revalidate_disk();
    assert!(state.disk_files.contains_key(&added));
    std::fs::remove_dir_all(nested).unwrap();
    state.revalidate_disk();
    assert!(!state.disk_files.contains_key(&added));
}

#[test]
fn fallback_refreshes_mutable_dependencies_and_package_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let app = directory.path().join("app");
    std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
    for (name, source) in [
        ("dep", "class Original {}"),
        ("replacement", "class Replacement {}"),
    ] {
        let lib = directory.path().join(name).join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("api.dart"), source).unwrap();
    }
    let config = app.join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let packages = app.join(".dart_tool/package_config.json");
    std::fs::write(
        &packages,
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    let local = directory.path().join("local.dart");
    std::fs::write(&local, "class LocalOriginal {}").unwrap();
    let mut state = LspState::new_from(Some(config), |_| None, app.clone());
    state.set_dynamic_file_watching(false);
    state.resolve_semantics = true;
    let uri = format!("file://{}", app.join("main.dart").display());
    state.open(
        &uri,
        "import 'package:dep/api.dart'; import '../local.dart'; void main() {}".into(),
        Some(1),
    );
    let dependency = directory.path().join("dep/lib/api.dart");
    assert!(state.fallback_stamps.contains_key(&dependency));
    let resets = state.external_reset_count;
    std::fs::write(&dependency, "class ChangedDependency {}").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_reset_count > resets);
    assert!(state.external_libraries.libraries().iter().any(|library| {
        library
            .files
            .iter()
            .any(|file| file.parsed.source.contains("ChangedDependency"))
    }));
    assert!(state.fallback_stamps.contains_key(&local));
    std::fs::write(&local, "class LocalChangedOnDisk {}").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_libraries.libraries().iter().any(|library| {
        library
            .files
            .iter()
            .any(|file| file.parsed.source.contains("LocalChangedOnDisk"))
    }));
    std::fs::write(
        &packages,
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../replacement","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_libraries.libraries().iter().any(|library| {
        library
            .files
            .iter()
            .any(|file| file.parsed.source.contains("Replacement"))
    }));
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_refreshes_nested_manifests_without_cross_file_rules() {
    let (directory, mut state, uri) = workspace(false);
    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let manifest = nested.join("pubspec.yaml");
    std::fs::write(&manifest, "name: nested\n").unwrap();
    std::fs::write(nested.join("api.dart"), "class Nested {}").unwrap();
    std::fs::write(
        directory.path().join("falcon.json"),
        r#"{"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    state.reload_config();
    state.resolve_semantics = true;
    state.open(&uri, "void main() {}".into(), Some(1));
    assert!(state.fallback_stamps.contains_key(&manifest));
    let resets = state.external_reset_count;
    std::fs::write(&manifest, "name: nested\nversion: 1.2.3\n").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_reset_count > resets);
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_repairs_rejected_closed_sources_without_directory_changes() {
    let (directory, mut state, uri) = workspace(false);
    let rejected = directory.path().join("rejected.dart");
    std::fs::File::create(&rejected)
        .unwrap()
        .set_len(u64::from(u32::MAX) + 1)
        .unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    assert!(!state.disk_files.contains_key(&rejected));
    assert!(state.fallback_stamps.contains_key(&rejected));
    std::fs::write(&rejected, "class Repaired {}").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert_eq!(
        state.disk_files[&rejected].parsed.source.as_ref(),
        "class Repaired {}"
    );
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn disabled_rules_do_not_repeat_changed_directory_publications() {
    let (directory, mut state, uri) = workspace(false);
    state.open(&uri, "void main() {}".into(), Some(1));
    assert_ne!(state.project_discovery_watches, []);
    std::fs::write(
        directory.path().join("falcon.json"),
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    state.reload_config();
    assert!(!state.resolve_semantics);
    assert!(!state.cross_file_rules_enabled());
    std::fs::create_dir(directory.path().join("new_directory")).unwrap();
    let modified = std::fs::metadata(directory.path())
        .unwrap()
        .modified()
        .unwrap();
    std::fs::File::open(directory.path())
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    state.revalidate_disk();
    assert!(state.revalidate_disk().is_empty());
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_observes_rule_config_creation_deletion_and_equal_metadata_edits() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::create_dir(directory.path().join(".git")).unwrap();
    let mut state = LspState::new_from(None, |_| None, first.clone());
    state.initialize_workspace_roots(vec![first.clone(), second.clone()]);
    state.set_dynamic_file_watching(false);
    let uri = format!("file://{}", first.join("main.dart").display());
    state.open(&uri, "void main() {}".into(), Some(1));
    for root in [&first, &second] {
        let config = root.join("falcon.json");
        assert_eq!(state.fallback_config_stamps[&config], None);
        std::fs::write(&config, r#"{"linter":{"rules":{"recommended":false}}}"#).unwrap();
        assert!(!state.revalidate_disk().is_empty());
        assert_eq!(
            state
                .config_for_path(&root.join("main.dart"))
                .linter
                .rules
                .recommended,
            Some(false)
        );
        let modified = config.metadata().unwrap().modified().unwrap();
        let len = config.metadata().unwrap().len();
        std::fs::write(&config, r#"{"linter":{"rules":{"recommended":true }}}"#).unwrap();
        std::fs::File::open(&config)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(config.metadata().unwrap().len(), len);
        assert!(!state.revalidate_disk().is_empty());
        assert_eq!(
            state
                .config_for_path(&root.join("main.dart"))
                .linter
                .rules
                .recommended,
            Some(true)
        );
        std::fs::remove_file(&config).unwrap();
        assert!(!state.revalidate_disk().is_empty());
        assert!(state.revalidate_disk().is_empty());
    }
    let git_config = directory.path().join("falcon.json");
    std::fs::write(&git_config, r#"{"linter":{"rules":{"recommended":true}}}"#).unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert_eq!(state.config.linter.rules.recommended, Some(true));
    assert_eq!(
        state.additional_root_configs[0]
            .config
            .linter
            .rules
            .recommended,
        Some(true)
    );
    assert!(state.revalidate_disk().is_empty());
    let ancestor_config = directory.path().join(".falcon.json");
    assert!(state.fallback_config_stamps.contains_key(&ancestor_config));
    std::fs::write(&ancestor_config, "{}").unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_discovers_new_mutable_dependency_subdirectories() {
    let directory = tempfile::tempdir().unwrap();
    let app = directory.path().join("app");
    let dependency = directory.path().join("dep/lib");
    std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(&dependency).unwrap();
    std::fs::write(app.join("falcon.json"), "{}").unwrap();
    std::fs::write(
        app.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#,
    ).unwrap();
    std::fs::write(dependency.join("api.dart"), "export 'generated/new.dart';").unwrap();
    let mut state = LspState::new_from(Some(app.join("falcon.json")), |_| None, app.clone());
    state.set_dynamic_file_watching(false);
    state.resolve_semantics = true;
    let uri = format!("file://{}", app.join("main.dart").display());
    state.open(
        &uri,
        "import 'package:dep/api.dart'; void main() {}".into(),
        Some(1),
    );
    let resets = state.external_reset_count;
    std::fs::create_dir(dependency.join("generated")).unwrap();
    let generated = dependency.join("generated/new.dart");
    std::fs::write(&generated, "class Generated {}").unwrap();
    let modified = dependency.metadata().unwrap().modified().unwrap();
    std::fs::File::open(&dependency)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_reset_count > resets);
    assert!(
        state
            .external_libraries
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.path == generated)
    );
    assert!(state.revalidate_disk().is_empty());
    let sibling = dependency.join("generated/another.dart");
    std::fs::write(&sibling, "class Another {}").unwrap();
    let modified = generated
        .parent()
        .unwrap()
        .metadata()
        .unwrap()
        .modified()
        .unwrap();
    std::fs::File::open(generated.parent().unwrap())
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.revalidate_disk().is_empty());
}

#[cfg(unix)]
#[test]
fn fallback_observes_closed_source_read_permission_recovery() {
    use std::os::unix::fs::PermissionsExt;
    let (directory, mut state, uri) = workspace(false);
    let source = directory.path().join("permission.dart");
    std::fs::write(&source, "class Recovered {}").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o000)).unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    let reads = state.disk_read_count;
    let modified = source.metadata().unwrap().modified().unwrap();
    let len = source.metadata().unwrap().len();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(source.metadata().unwrap().modified().unwrap(), modified);
    assert_eq!(source.metadata().unwrap().len(), len);
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.disk_read_count > reads);
    assert!(state.disk_files.contains_key(&source));
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_admits_lazy_package_inputs_without_hiding_known_changes() {
    let (directory, mut state, uri) = workspace(false);
    let manifest = directory.path().join("pubspec.yaml");
    std::fs::write(&manifest, "name: example\n").unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    let external = tempfile::tempdir().unwrap();
    std::fs::write(external.path().join("pubspec.yaml"), "name: external\n").unwrap();
    let _ = state
        .environment
        .file_package(&external.path().join("lib/api.dart"));
    let resets = state.external_reset_count;
    assert!(state.revalidate_disk().is_empty());
    assert_eq!(state.external_reset_count, resets);
    assert!(state.revalidate_disk().is_empty());
    std::fs::write(&manifest, "name: example\nversion: 2.0.0\n").unwrap();
    let _ = state
        .environment
        .file_package(&external.path().join("another/api.dart"));
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.external_reset_count > resets);
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn rejected_buffers_do_not_trigger_idle_discovery_publications() {
    for cross_file_only in [false, true] {
        let (directory, mut state, uri) = workspace(false);
        if cross_file_only {
            std::fs::write(directory.path().join("falcon.json"),
                r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"error"}}}}"#).unwrap();
            state.reload_config();
            assert!(!state.resolve_semantics);
            assert!(state.cross_file_rules_enabled());
        }
        state.open(&uri, "void main() {}".into(), Some(1));
        state.documents.get_mut(&uri).unwrap().source_rejected = true;
        state.project_paths = None;
        state.invalidate_semantic_snapshot();
        let analyses = state.documents[&uri].analyze_count;
        assert!(state.revalidate_disk().is_empty());
        assert!(state.revalidate_disk().is_empty());
        assert_eq!(state.documents[&uri].analyze_count, analyses);
    }
}

#[test]
fn rejected_watcher_registration_reloads_previously_loaded_local_sources() {
    let (_directory, mut state, uri) = workspace(true);
    let outside = tempfile::tempdir().unwrap();
    let source = outside.path().join("api.dart");
    std::fs::write(&source, "class BeforeRejection {}").unwrap();
    let text = format!(
        "import '../{}/api.dart'; void main() {{}}",
        outside.path().file_name().unwrap().to_str().unwrap(),
    );
    state.open(&uri, text, Some(1));
    assert!(
        state
            .external_libraries
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.parsed.source.contains("BeforeRejection"))
    );
    std::fs::write(&source, "class ChangedBeforeFallback {}").unwrap();
    state.set_dynamic_file_watching(false);
    assert!(!state.revalidate_disk().is_empty());
    assert!(
        state
            .external_libraries
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.parsed.source.contains("ChangedBeforeFallback"))
    );
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn deprecated_external_api_does_not_cause_idle_package_discovery_resets() {
    let (directory, mut state, uri) = workspace(false);
    std::fs::write(
        directory.path().join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false,"correctness":{"deprecated-member-use-from-same-package":"error"}}},"cross-file":{"enabled":false}}"#,
    ).unwrap();
    state.reload_config();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("pubspec.yaml"), "name: external\n").unwrap();
    std::fs::write(
        outside.path().join("api.dart"),
        "@deprecated void oldApi() {}",
    )
    .unwrap();
    let text = format!(
        "import '../{}/api.dart'; void main() {{ oldApi(); }}",
        outside.path().file_name().unwrap().to_str().unwrap(),
    );
    state.open(&uri, text, Some(1));
    assert!(
        state
            .external_libraries
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.parsed.source.contains("oldApi"))
    );
    let resets = state.external_reset_count;
    assert!(state.revalidate_disk().is_empty());
    assert!(state.revalidate_disk().is_empty());
    assert_eq!(state.external_reset_count, resets);
}

#[test]
fn fallback_observes_workspace_member_rule_config_candidates() {
    for declaration in ["pub", "melos"] {
        for initially_present in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let member = root.join("packages/app");
            std::fs::create_dir_all(&member).unwrap();
            std::fs::write(
                root.join("pubspec.yaml"),
                if declaration == "pub" {
                    "name: root\nworkspace: [packages/app]\n"
                } else {
                    "name: root\n"
                },
            )
            .unwrap();
            if declaration == "melos" {
                std::fs::write(root.join("melos.yaml"), "packages: ['packages/*']\n").unwrap();
            }
            std::fs::write(
                member.join("pubspec.yaml"),
                "name: app\nresolution: workspace\n",
            )
            .unwrap();
            std::fs::write(
                root.join("falcon.json"),
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
            )
            .unwrap();
            let config = member.join("falcon.json");
            let warning = r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"warn"}}},"cross-file":{"enabled":false}}"#;
            let error = r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"enabled":false}}"#;
            if initially_present {
                std::fs::write(&config, warning).unwrap();
            }
            let mut state = LspState::new_from(None, |_| None, root.to_path_buf());
            state.initialize_workspace_roots(vec![root.to_path_buf()]);
            state.set_dynamic_file_watching(false);
            assert!(state.fallback_config_stamps.contains_key(&config));
            let uri = format!("file://{}", member.join("main.dart").display());
            let initial = state.open(
                &uri,
                "void main() { dynamic value = 1; print(value); }".into(),
                Some(1),
            );
            assert_eq!(
                initial
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-dynamic"),
                initially_present
            );
            std::fs::write(&config, error).unwrap();
            let updates = state.revalidate_disk();
            assert!(
                updates
                    .iter()
                    .flat_map(|(_, diagnostics)| diagnostics)
                    .any(|diagnostic| {
                        diagnostic.rule == "avoid-dynamic"
                            && diagnostic.severity == falcon_diagnostics::Severity::Error
                    })
            );
            assert!(!state.cross_file_rules_enabled());
            assert!(state.revalidate_disk().is_empty());
            std::fs::remove_file(&config).unwrap();
            let updates = state.revalidate_disk();
            assert!(!updates.is_empty());
            assert!(
                updates
                    .iter()
                    .flat_map(|(_, diagnostics)| diagnostics)
                    .all(|diagnostic| diagnostic.rule != "avoid-dynamic")
            );
            assert!(state.revalidate_disk().is_empty());
            std::fs::write(root.join("packages/falcon.json"), error).unwrap();
            assert!(
                state
                    .revalidate_disk()
                    .iter()
                    .flat_map(|(_, diagnostics)| diagnostics)
                    .any(|diagnostic| diagnostic.rule == "avoid-dynamic")
            );
            assert!(state.revalidate_disk().is_empty());
        }
    }
}

#[test]
fn fallback_saves_outside_project_membership_do_not_repeat_idle_publications() {
    for rejected_buffer in [false, true] {
        for excluded in [false, true] {
            let (directory, mut state, uri) = workspace(false);
            if !rejected_buffer {
                std::fs::write(
                    directory.path().join("falcon.json"),
                    r#"{"cross-file":{"enabled":false}}"#,
                )
                .unwrap();
                state.reload_config();
                assert!(!state.cross_file_rules_enabled());
            }
            let outside = tempfile::tempdir().unwrap();
            let other = if excluded {
                directory.path().join("excluded.dart")
            } else {
                outside.path().join("outside.dart")
            };
            std::fs::write(&other, "void other() {}").unwrap();
            if excluded {
                state
                    .config
                    .files
                    .includes
                    .extend_excludes(&[other.to_string_lossy().into_owned()]);
            }
            state.open(&uri, "void main() {}".into(), Some(1));
            let other_uri = format!("file://{}", other.display());
            state.open(&other_uri, "void other() {}".into(), Some(1));
            if rejected_buffer {
                let rejected_uri = format!(
                    "file://{}",
                    directory.path().join("rejected.dart").display()
                );
                state.open(&rejected_uri, String::new(), Some(1));
                state
                    .documents
                    .get_mut(&rejected_uri)
                    .unwrap()
                    .source_rejected = true;
            }
            assert!(state.semantic_snapshot.is_some());
            assert!(state.project_paths.is_some());
            state.save(&other_uri, None);
            assert!(state.project_paths.is_some());
            let analyses: u64 = state.documents.values().map(|doc| doc.analyze_count).sum();
            assert!(state.revalidate_disk().is_empty());
            assert!(state.revalidate_disk().is_empty());
            assert_eq!(
                state
                    .documents
                    .values()
                    .map(|doc| doc.analyze_count)
                    .sum::<u64>(),
                analyses
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn fallback_observes_ctime_only_source_stamp_changes() {
    use std::os::unix::fs::MetadataExt;
    let (directory, mut state, uri) = workspace(false);
    let source = directory.path().join("closed.dart");
    std::fs::write(&source, "class Closed {}").unwrap();
    state.open(&uri, "void main() {}".into(), Some(1));
    let metadata = source.metadata().unwrap();
    let previous = state
        .fallback_stamps
        .get_mut(&source)
        .unwrap()
        .as_mut()
        .unwrap();
    assert_eq!(previous.changed, (metadata.ctime(), metadata.ctime_nsec()));
    // Simulate the preceding owner/ACL revision without changing length, mode or mtime.
    previous.changed.0 -= 1;
    let reads = state.disk_read_count;
    assert!(!state.revalidate_disk().is_empty());
    assert!(state.disk_read_count > reads);
    assert!(state.revalidate_disk().is_empty());
}

#[test]
fn fallback_preserves_pre_discovery_build_failure_until_source_event() {
    let (directory, mut state, uri) = workspace(false);
    let source = directory.path().join("closed.dart");
    std::fs::write(&source, "class Before {}").unwrap();
    state.build_panic = true;
    let initial = state.open(&uri, "void main() {}".into(), Some(1));
    assert!(initial.iter().any(|diagnostic| {
        diagnostic.rule == falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
    }));
    assert!(state.project_paths.is_none());
    assert!(state.semantic_snapshot.is_none());
    let error = state.semantic_snapshot_error.as_ref().unwrap().to_string();
    assert!(error.contains("semantic build regression"));

    state.build_panic = false;
    let repeated = state.analyze(&uri);
    assert!(repeated.iter().any(|diagnostic| {
        diagnostic.rule == falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
    }));
    assert_eq!(
        state.semantic_snapshot_error.as_ref().unwrap().to_string(),
        error
    );
    assert!(state.project_paths.is_none());
    assert!(state.semantic_snapshot.is_none());
    let analyses = state.documents[&uri].analyze_count;
    assert!(state.revalidate_disk().is_empty());
    assert!(state.revalidate_disk().is_empty());
    assert_eq!(state.documents[&uri].analyze_count, analyses);
    assert_eq!(
        state.semantic_snapshot_error.as_ref().unwrap().to_string(),
        error
    );
    assert!(state.semantic_snapshot.is_none());

    std::fs::write(&source, "class After {}").unwrap();
    let recovered = state.watched_files_changed(std::slice::from_ref(&source));
    assert!(!recovered.is_empty());
    assert!(
        recovered
            .iter()
            .flat_map(|(_, diagnostics)| diagnostics)
            .all(|diagnostic| {
                diagnostic.rule != falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
            })
    );
    assert!(state.semantic_snapshot_error.is_none());
    assert!(state.semantic_snapshot.is_some());
    assert!(state.project_paths.is_some());
    assert!(state.revalidate_disk().is_empty());
}
