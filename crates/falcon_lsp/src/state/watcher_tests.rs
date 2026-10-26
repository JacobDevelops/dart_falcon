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
    state.watched_files_changed(&[nested.clone()]);
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
            assert!(state.project_discovery_watches.is_empty());
            assert!(state.disk_files[&closed].fallback_stamp.is_none());
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
