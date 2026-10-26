use super::*;

fn fixture(api: &str) -> (tempfile::TempDir, LspState, String, String) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let config = root.join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    std::fs::write(root.join("api.dart"), api).unwrap();
    let caller = "import 'api.dart';\nvoid main() { api(); }\n";
    std::fs::write(root.join("main.dart"), caller).unwrap();
    let api_uri = format!("file://{}", root.join("api.dart").display());
    let caller_uri = format!("file://{}", root.join("main.dart").display());
    let mut state = LspState::new_from(Some(config), |_| None, root.to_path_buf());
    state.open(&caller_uri, caller.into(), Some(1));
    assert!(state.semantic_snapshot.is_some());
    (directory, state, api_uri, caller_uri)
}

fn counters(state: &LspState) -> (u64, u64, u64, u64, u64) {
    (
        state.semantic_snapshot_build_count,
        state.semantic_topology_build_count,
        state.external_index_build_count,
        state.workspace_walk_count,
        state.disk_parse_count,
    )
}

fn disk_source<'a>(state: &'a LspState, uri: &str) -> &'a str {
    let path = canonical_or_lexical(&uri_to_path(uri));
    let snapshot = state.semantic_snapshot.as_ref().unwrap();
    snapshot.files[snapshot.by_path[&path]].source()
}

#[test]
fn equal_api_open_save_and_unsaved_close_reuse_indexes_and_restore_disk() {
    let original = "int api() => 1;\n";
    let (_directory, mut state, api_uri, caller_uri) = fixture(original);
    let before = counters(&state);
    state.open(&api_uri, "int api() => 222;\n".into(), Some(1));
    assert_eq!(counters(&state), before);
    assert_eq!(state.affected_open_uris(&api_uri), vec![api_uri.clone()]);
    state.save(&api_uri, Some("int api() => 3333;\n".into()));
    assert_eq!(counters(&state), before);
    assert_eq!(state.affected_open_uris(&api_uri), vec![api_uri.clone()]);
    state.close(&api_uri);
    state.analyze(&caller_uri);
    let mut after_close = before;
    after_close.4 += 1;
    assert_eq!(counters(&state), after_close);
    assert_eq!(disk_source(&state, &api_uri), original);
    assert!(!state.documents.contains_key(&api_uri));
}

#[test]
fn close_reuses_unchanged_disk_parse_and_checks_only_closing_file() {
    let original = "int api() => 1;\n";
    let (_directory, mut state, api_uri, caller_uri) = fixture(original);
    let before = counters(&state);
    let reads = state.disk_read_count;
    state.open(&api_uri, original.into(), Some(1));
    state.close(&api_uri);
    state.analyze(&caller_uri);
    assert_eq!(counters(&state), before);
    assert_eq!(state.disk_read_count, reads + 1);
}

#[test]
fn close_restores_disk_api_and_reanalyzes_dependents() {
    let original = "void api() {}\n";
    let (_directory, mut state, api_uri, caller_uri) = fixture(original);
    assert!(state.analyze(&caller_uri).is_empty());
    state.open(&api_uri, "int api() => 1;\n".into(), Some(1));
    assert!(state.affected_open_uris(&api_uri).contains(&caller_uri));
    assert!(
        state
            .analyze(&caller_uri)
            .iter()
            .any(|diagnostic| { diagnostic.rule == "avoid-ignoring-return-values" })
    );
    let builds = state.semantic_snapshot_build_count;
    state.close(&api_uri);
    assert!(state.semantic_snapshot.is_none());
    assert!(state.affected_open_uris(&api_uri).contains(&caller_uri));
    assert!(state.analyze(&caller_uri).is_empty());
    assert_eq!(state.semantic_snapshot_build_count, builds + 1);
    assert_eq!(disk_source(&state, &api_uri), original);
}

#[test]
fn close_reads_actual_disk_even_without_a_delivered_watch_event() {
    let original = "int api() => 1;\n";
    let (directory, mut state, api_uri, caller_uri) = fixture(original);
    state.open(&api_uri, original.into(), Some(1));
    let before = counters(&state);
    let changed = "int api() => 2222;\n";
    std::fs::write(directory.path().join("api.dart"), changed).unwrap();
    state.close(&api_uri);
    state.analyze(&caller_uri);
    assert_eq!(state.semantic_snapshot_build_count, before.0);
    assert_eq!(state.workspace_walk_count, before.3);
    assert_eq!(state.disk_parse_count, before.4 + 1);
    assert_eq!(disk_source(&state, &api_uri), changed);
}

#[test]
fn close_inferred_return_body_change_rebuilds_indexes() {
    let original = "api() => 1;\n";
    let (_directory, mut state, api_uri, caller_uri) = fixture(original);
    state.open(&api_uri, "api() => 'changed';\n".into(), Some(1));
    let builds = state.semantic_snapshot_build_count;
    state.close(&api_uri);
    assert!(state.semantic_snapshot.is_none());
    state.analyze(&caller_uri);
    assert_eq!(state.semantic_snapshot_build_count, builds + 1);
    assert_eq!(disk_source(&state, &api_uri), original);
}

#[test]
fn watched_disk_change_remains_authoritative_after_close() {
    let original = "void api() {}\n";
    let (directory, mut state, api_uri, caller_uri) = fixture(original);
    state.open(&api_uri, original.into(), Some(1));
    state.close(&api_uri);
    let path = directory.path().join("api.dart");
    std::fs::write(&path, "int api() => 1;\n").unwrap();
    let builds = state.semantic_snapshot_build_count;
    state.watched_files_changed(&[path]);
    assert_eq!(state.semantic_snapshot_build_count, builds + 1);
    assert!(
        state
            .analyze(&caller_uri)
            .iter()
            .any(|diagnostic| { diagnostic.rule == "avoid-ignoring-return-values" })
    );
}

#[test]
fn closing_deleted_file_does_not_keep_buffer_in_snapshot() {
    let original = "void api() {}\n";
    let (directory, mut state, api_uri, caller_uri) = fixture(original);
    state.open(&api_uri, original.into(), Some(1));
    let path = canonical_or_lexical(&directory.path().join("api.dart"));
    std::fs::remove_file(&path).unwrap();
    state.close(&api_uri);
    assert!(state.semantic_snapshot.is_none());
    state.analyze(&caller_uri);
    let snapshot = state.semantic_snapshot.as_ref().unwrap();
    assert!(!snapshot.by_path.contains_key(&path));
    assert!(!state.disk_files.contains_key(&path));
}
