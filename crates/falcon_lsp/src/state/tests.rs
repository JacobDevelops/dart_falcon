use super::*;

#[test]
fn invalid_secondary_root_edit_retains_last_good_rules() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for root in [&first, &second] {
        std::fs::create_dir(root).unwrap();
    }
    std::fs::write(
        first.join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let config = second.join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let mut state = LspState::new_from(None, |_| None, first.clone());
    state.initialize_workspace_roots(vec![first.clone(), second.clone()]);
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    let source = "void main() { dynamic value = 1; print(value); }\n";
    let primary_uri = format!("file://{}", first.join("main.dart").display());
    let secondary_uri = format!("file://{}", second.join("main.dart").display());
    assert!(
        state
            .open(&primary_uri, source.to_owned(), Some(1))
            .is_empty()
    );
    let before = state.open(&secondary_uri, source.to_owned(), Some(1));
    assert!(
        before.iter().any(|diagnostic| {
            diagnostic.rule == "avoid-dynamic"
                && diagnostic.severity == falcon_diagnostics::Severity::Error
        }),
        "{before:?}"
    );

    std::fs::write(&config, r#"{"linter":{"enabledd":false}}"#).unwrap();
    let refreshed = state.watched_files_changed(std::slice::from_ref(&config));
    let messages = state.take_config_messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0].typ, MessageType::ERROR);
    assert!(messages[0].message.contains(config.to_str().unwrap()));
    let (_, after) = refreshed
        .iter()
        .find(|(uri, _)| uri == &secondary_uri)
        .unwrap();
    assert!(
        after.iter().any(|diagnostic| {
            diagnostic.rule == "avoid-dynamic"
                && diagnostic.severity == falcon_diagnostics::Severity::Error
        }),
        "{after:?}"
    );
    assert!(state.analyze(&primary_uri).is_empty());
    assert!(state.analyze(&secondary_uri).iter().any(|diagnostic| {
        diagnostic.rule == "avoid-dynamic"
            && diagnostic.severity == falcon_diagnostics::Severity::Error
    }));
    state.watched_files_changed(&[config]);
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    assert!(state.analyze(&secondary_uri).iter().any(|diagnostic| {
        diagnostic.rule == "avoid-dynamic"
            && diagnostic.severity == falcon_diagnostics::Severity::Error
    }));
}

#[test]
fn secondary_root_cross_file_rules_run_without_enabling_primary_root_rules() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for root in [&first, &second] {
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("lib/main.dart"), "void main() {}\n").unwrap();
        std::fs::write(root.join("lib/orphan.dart"), "class Orphan {}\n").unwrap();
    }
    std::fs::write(
        first.join("falcon.json"),
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    std::fs::write(
        second.join("falcon.json"),
        r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"error"}}}}"#,
    ).unwrap();
    let mut state = LspState::new_from(None, |_| None, first.clone());
    state.initialize_workspace_roots(vec![first.clone(), second.clone()]);
    assert!(state.cross_file_rules_enabled());
    let findings = state.cross_file_diagnostics();
    assert!(
        findings.values().flatten().any(|diagnostic| {
            diagnostic.rule == "unused-files"
                && diagnostic.file_path == second.join("lib/orphan.dart").to_string_lossy()
                && diagnostic.severity == falcon_diagnostics::Severity::Error
        }),
        "{findings:?}"
    );
    assert!(
        findings
            .keys()
            .all(|path| !Path::new(path).starts_with(&first))
    );
}

#[test]
fn cross_file_usage_in_another_editor_root_does_not_suppress_unused_files() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for root in [&first, &second] {
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(
            root.join("falcon.json"),
            r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
        )
        .unwrap();
        std::fs::write(root.join("lib/main.dart"), "void main() {}").unwrap();
    }
    let orphan = second.join("lib/orphan.dart");
    std::fs::write(&orphan, "class Orphan {}").unwrap();
    std::fs::write(
        first.join("lib/main.dart"),
        "import '../../second/lib/orphan.dart'; void main() { Orphan(); }",
    )
    .unwrap();
    let mut standalone = LspState::new_from(None, |_| None, second.clone());
    standalone.initialize_workspace_roots(vec![second.clone()]);
    let expected = standalone.cross_file_diagnostics();
    assert!(
        expected[orphan.to_str().unwrap()]
            .iter()
            .any(|diagnostic| { diagnostic.rule == "unused-files" })
    );
    let mut editor = LspState::new_from(None, |_| None, first.clone());
    editor.initialize_workspace_roots(vec![first, second]);
    assert_eq!(
        serde_json::to_value(editor.cross_file_diagnostics()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn nested_workspace_roots_remain_inputs_to_the_parent_cross_file_run() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let nested = root.join("nested");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        root.join("falcon.json"),
        r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
    )
    .unwrap();
    std::fs::write(root.join("lib/used.dart"), "class Used {}").unwrap();
    std::fs::write(
        nested.join("main.dart"),
        "import '../lib/used.dart'; void main() { Used(); }",
    )
    .unwrap();
    let mut standalone = LspState::new_from(None, |_| None, root.to_path_buf());
    standalone.initialize_workspace_roots(vec![root.to_path_buf()]);
    assert!(standalone.cross_file_diagnostics().is_empty());
    let mut editor = LspState::new_from(None, |_| None, root.to_path_buf());
    editor.initialize_workspace_roots(vec![root.to_path_buf(), nested]);
    assert!(editor.cross_file_diagnostics().is_empty());
}

#[test]
fn invalid_member_config_preserves_valid_workspace_and_member_rules() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: root\nworkspace:\n  - good\n  - broken\n",
    )
    .unwrap();
    std::fs::write(
        root.join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    for member in ["good", "broken"] {
        std::fs::create_dir(root.join(member)).unwrap();
        std::fs::write(
            root.join(member).join("pubspec.yaml"),
            format!("name: {member}\nresolution: workspace\n"),
        )
        .unwrap();
    }
    let valid = root.join("good/falcon.json");
    let invalid = root.join("broken/falcon.json");
    std::fs::write(
        &valid,
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"enabled":false}}"#,
    ).unwrap();
    std::fs::write(&invalid, "{").unwrap();
    let mut state = LspState::new_from(None, |_| None, root.to_path_buf());
    let messages = state.take_config_messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0].typ, MessageType::ERROR);
    assert!(messages[0].message.contains(invalid.to_str().unwrap()));
    let source = "void main() { dynamic value = 1; print(value); }\n";
    let good_uri = format!("file://{}", root.join("good/main.dart").display());
    let broken_uri = format!("file://{}", root.join("broken/main.dart").display());
    assert!(
        state
            .open(&good_uri, source.to_owned(), Some(1))
            .iter()
            .any(|diagnostic| {
                diagnostic.rule == "avoid-dynamic"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error
            })
    );
    assert!(
        state
            .open(&broken_uri, source.to_owned(), Some(1))
            .is_empty()
    );
    state.reload_config();
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    assert!(
        state
            .analyze(&good_uri)
            .iter()
            .any(|diagnostic| diagnostic.rule == "avoid-dynamic")
    );
}

#[test]
fn starting_inside_non_git_workspace_member_preserves_shared_loader_root_and_overrides() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let member = root.join("packages/member");
    let start = member.join("lib/nested");
    std::fs::create_dir_all(&start).unwrap();
    std::fs::create_dir(root.join("shared")).unwrap();
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: root\nworkspace:\n  - packages/member\n",
    )
    .unwrap();
    std::fs::write(
        member.join("pubspec.yaml"),
        "name: member\nresolution: workspace\n",
    )
    .unwrap();
    std::fs::write(
        root.join("falcon.json"),
        r#"{"files":{"includes":["shared/**"]},"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    std::fs::write(
        member.join("falcon.json"),
        r#"{"files":{"includes":["lib/**"]},"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let shared = root.join("shared/shared.dart");
    std::fs::write(&shared, "void shared() {}\n").unwrap();
    let path = member.join("lib/main.dart");
    let source = "void main() { dynamic value = 1; print(value); }\n";
    std::fs::write(&path, source).unwrap();
    let mut state = LspState::new_from(None, |_| None, start);
    assert_eq!(state.workspace_root, canonical_or_lexical(root));
    assert_eq!(
        state.config.root_dir(),
        Some(canonical_or_lexical(root).as_path())
    );
    let diagnostics = state.open(
        &format!("file://{}", path.display()),
        source.to_owned(),
        Some(1),
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.rule == "avoid-dynamic"
            && diagnostic.severity == falcon_diagnostics::Severity::Error
    }));
    let (files, complete) = state.collect_project_files(ProjectView::Semantic);
    assert!(complete);
    assert!(files.iter().any(|file| file.path == shared));
    assert!(files.iter().any(|file| file.path == path));
    state.reload_config();
    assert_eq!(state.workspace_root, canonical_or_lexical(root));
}

#[test]
fn initialized_roots_report_shared_ancestor_configuration_error_once() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join(".git")).unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, r#"{"linter":{"enabledd":true}}"#).unwrap();
    let mut state = LspState::new_with_sdk_locator(None, |_| None);
    state.initialize_workspace_roots(vec![first, second]);
    let messages = state.take_config_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].typ, MessageType::ERROR);
    assert!(messages[0].message.contains(config.to_str().unwrap()));
    state.reload_config();
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    std::fs::write(&config, "{}").unwrap();
    state.reload_config();
    state.take_config_messages();
    std::fs::write(&config, r#"{"linter":{"enabledd":true}}"#).unwrap();
    state.reload_config();
    assert_eq!(state.take_config_messages().len(), 1);
}

#[test]
fn workspace_declaration_events_rebuild_effective_rule_registry() {
    for declaration in ["melos.yaml", "pubspec.yaml"] {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config.clone()), |_| None);
        let uri = format!("file://{}", directory.path().join("main.dart").display());
        let source = "void main() { dynamic value = 1; print(value); }\n";
        assert!(state.open(&uri, source.to_owned(), Some(1)).is_empty());
        let parses = state.document(&uri).unwrap().parse_count;
        std::fs::write(
            &config,
            r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        let refreshed = state.watched_files_changed(&[directory.path().join(declaration)]);
        assert_eq!(refreshed.len(), 1);
        assert!(refreshed[0].1.iter().any(|diagnostic| {
            diagnostic.rule == "avoid-dynamic"
                && diagnostic.severity == falcon_diagnostics::Severity::Error
        }));
        assert_eq!(state.document(&uri).unwrap().parse_count, parses);
    }
}

#[test]
fn initialized_roots_replace_cwd_discovery_and_report_each_config_error() {
    let directory = tempfile::tempdir().unwrap();
    let cwd = directory.path().join("cwd");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for root in [&cwd, &first, &second] {
        std::fs::create_dir(root).unwrap();
        std::fs::write(root.join("main.dart"), "void main() {}\n").unwrap();
    }
    std::fs::write(cwd.join("falcon.json"), "{").unwrap();
    std::fs::write(
        first.join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let invalid = second.join("falcon.json");
    std::fs::write(&invalid, r#"{"linter":{"enabledd":false}}"#).unwrap();
    let mut state = LspState::new_from(None, |_| None, cwd.clone());
    state.initialize_workspace_roots(vec![first.clone(), second.clone(), first.clone()]);
    assert_eq!(state.workspace_root, canonical_or_lexical(&first));
    assert_eq!(state.workspace_roots.len(), 2);
    assert_eq!(state.config.linter.rules.recommended, Some(false));
    let messages = state.take_config_messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0].typ, MessageType::ERROR);
    assert!(messages[0].message.contains(&invalid.display().to_string()));
    for view in [ProjectView::Semantic, ProjectView::CrossFile] {
        let (files, complete) = state.collect_project_files(view);
        assert!(complete);
        assert_eq!(files.len(), 2);
        for root in [&first, &second] {
            assert!(files.iter().any(|file| file.path == root.join("main.dart")));
        }
        assert!(files.iter().all(|file| !file.path.starts_with(&cwd)));
    }
    state.reload_config();
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    assert_eq!(state.workspace_roots.len(), 2);
    std::fs::write(&invalid, "{}").unwrap();
    state.watched_files_changed(std::slice::from_ref(&invalid));
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    std::fs::write(&invalid, "{").unwrap();
    state.watched_files_changed(&[invalid]);
    assert_eq!(state.take_config_messages().len(), 1);
}

#[test]
fn initialized_roots_use_independent_rules_and_file_includes_after_reload() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    let source = "void main() { dynamic value = 1; print(value); }\n";
    for (root, included, severity) in [(&first, "lib", "warn"), (&second, "src", "error")] {
        for folder in ["lib", "src"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(root.join(folder).join("main.dart"), source).unwrap();
        }
        std::fs::write(
            root.join("falcon.json"),
            format!(
                r#"{{"files":{{"includes":["{included}/**"]}},"linter":{{"rules":{{"recommended":false,"suspicious":{{"avoid-dynamic":"{severity}"}}}}}},"cross-file":{{"enabled":false}}}}"#
            ),
        )
        .unwrap();
    }
    let mut state = LspState::new_from(None, |_| None, first.clone());
    state.initialize_workspace_roots(vec![first.clone(), second.clone()]);
    let first_uri = format!("file://{}", first.join("lib/main.dart").display());
    let second_uri = format!("file://{}", second.join("src/main.dart").display());
    for (uri, severity) in [
        (&first_uri, falcon_diagnostics::Severity::Warning),
        (&second_uri, falcon_diagnostics::Severity::Error),
    ] {
        let diagnostics = state.open(uri, source.to_owned(), Some(1));
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == "avoid-dynamic" && diagnostic.severity == severity
            }),
            "{diagnostics:?}"
        );
    }
    for view in [ProjectView::Semantic, ProjectView::CrossFile] {
        let (files, complete) = state.collect_project_files(view);
        assert!(complete);
        let paths: HashSet<_> = files.into_iter().map(|file| file.path).collect();
        assert_eq!(
            paths,
            HashSet::from([
                canonical_or_lexical(&first.join("lib/main.dart")),
                canonical_or_lexical(&second.join("src/main.dart")),
            ])
        );
    }
    let excluded_uri = format!("file://{}", second.join("lib/main.dart").display());
    assert!(
        state
            .open(&excluded_uri, source.to_owned(), Some(1))
            .is_empty()
    );
    std::fs::write(
        second.join("falcon.json"),
        r#"{"files":{"includes":["src/**"]},"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    state.reload_config();
    assert!(state.analyze(&second_uri).is_empty());
    assert!(state.analyze(&first_uri).iter().any(|diagnostic| {
        diagnostic.rule == "avoid-dynamic"
            && diagnostic.severity == falcon_diagnostics::Severity::Warning
    }));
    for view in [ProjectView::Semantic, ProjectView::CrossFile] {
        let semantic = matches!(view, ProjectView::Semantic);
        let (files, complete) = state.collect_project_files(view);
        assert!(complete);
        let excluded = canonical_or_lexical(&second.join("lib/main.dart"));
        assert_eq!(
            files.iter().any(|file| file.path == excluded),
            semantic,
            "explicitly open excluded buffers belong only to the semantic view"
        );
    }
}

#[test]
fn initialized_root_does_not_expand_to_ancestor_configuration_directory() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join(".git")).unwrap();
    std::fs::write(directory.path().join("falcon.json"), "{}").unwrap();
    std::fs::write(directory.path().join("outside.dart"), "void outside() {}").unwrap();
    let root = directory.path().join("nested");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("inside.dart"), "void inside() {}").unwrap();
    let mut state = LspState::new_from(None, |_| None, directory.path().to_path_buf());
    state.initialize_workspace_roots(vec![root.clone()]);
    let (files, complete) = state.collect_project_files(ProjectView::Semantic);
    assert!(complete);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, root.join("inside.dart"));
    state.reload_config();
    assert_eq!(state.workspace_root, canonical_or_lexical(&root));
}

#[test]
fn edit_reuses_workspace_disk_parses_and_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let main = directory.path().join("main.dart");
    let support = directory.path().join("support.dart");
    let dependent = directory.path().join("dependent.dart");
    let dependent_source = "import 'main.dart'; void dependent() { main(); }";
    std::fs::write(&dependent, dependent_source).unwrap();
    std::fs::write(&main, "import 'support.dart'; void main() {}").unwrap();
    std::fs::write(&support, "class Before {}").unwrap();
    let uri = format!("file://{}", main.display());
    let dependent_uri = format!("file://{}", dependent.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    state.open(&uri, std::fs::read_to_string(&main).unwrap(), Some(1));
    state.open(&dependent_uri, dependent_source.into(), Some(1));
    state.cross_file_pass();
    let snapshots = state.semantic_snapshot_build_count;
    let topologies = state.semantic_topology_build_count;
    let external_indexes = state.external_index_build_count;
    let cross_file_indexes = state.cross_file_index_build_count;
    let dependent_analyses = state.document(&dependent_uri).unwrap().analyze_count;
    let document_analyses = state.document(&uri).unwrap().analyze_count;
    let parses = state.disk_parse_count;
    let reads = state.disk_read_count;
    let walks = state.workspace_walk_count;
    let document_parses = state.document(&uri).unwrap().parse_count;
    // Changed disk metadata must not be polled during an unrelated buffer edit.
    std::fs::write(&support, "class AfterDiskChange {}").unwrap();
    state.change(
        &uri,
        "import 'support.dart'; void main() { print(1); }".into(),
        Some(2),
    );
    let analyzed = state.analyze_affected_many(std::slice::from_ref(&uri));
    assert_eq!(analyzed.len(), 1, "{analyzed:?}");
    assert_eq!(analyzed[0].0, uri);
    state.cross_file_pass();
    assert_eq!(state.semantic_snapshot_build_count, snapshots);
    assert_eq!(state.semantic_topology_build_count, topologies);
    assert_eq!(state.external_index_build_count, external_indexes);
    assert_eq!(state.cross_file_index_build_count, cross_file_indexes);
    assert_eq!(
        state.document(&dependent_uri).unwrap().analyze_count,
        dependent_analyses
    );
    assert_eq!(
        state.document(&uri).unwrap().analyze_count,
        document_analyses + 1
    );
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        "class Before {}",
        "disk changes enter the cache only through lifecycle or watcher events"
    );
    assert_eq!(
        state.document(&uri).unwrap().parse_count,
        document_parses + 1
    );
    assert_eq!(
        state.disk_parse_count, parses,
        "unchanged disk sources were reparsed"
    );
    assert_eq!(
        state.disk_read_count, reads,
        "edit performed disk source reads or their metadata checks"
    );
    assert_eq!(
        state.workspace_walk_count, walks,
        "edit walked the workspace again"
    );
    state.watched_files_changed(std::slice::from_ref(&support));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        "class AfterDiskChange {}"
    );
    assert_eq!(state.disk_read_count, reads + 1);
    assert_eq!(state.disk_parse_count, parses + 1);
}

#[test]
fn return_type_edits_rebuild_indexes_and_refresh_dependent_diagnostics() {
    for (before, after) in [
        ("void api() {}", "int api() => 1;"),
        (
            // Cross-file inference supports variable initializers, whereas
            // unannotated top-level function returns remain dynamic.
            "void sink() {} int value() => 1; final api = sink;",
            "void sink() {} int value() => 1; final api = value;",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{
                "linter": { "rules": {
                    "recommended": false,
                    "suspicious": { "avoid-ignoring-return-values": "warn" }
                } },
                "cross-file": { "rules": {
                    "recommended": false,
                    "correctness": { "unused-code": "warn" }
                } }
            }"#,
        )
        .unwrap();
        let api = directory.path().join("api.dart");
        let dependent = directory.path().join("main.dart");
        let dependent_source = "import 'api.dart'; void main() { api(); }";
        std::fs::write(&api, before).unwrap();
        std::fs::write(&dependent, dependent_source).unwrap();
        let api_uri = format!("file://{}", api.display());
        let dependent_uri = format!("file://{}", dependent.display());
        let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
        state.resolve_semantics = true;
        state.open(&api_uri, before.into(), Some(1));
        let initial = state.open(&dependent_uri, dependent_source.into(), Some(1));
        assert!(
            !initial
                .iter()
                .any(|diagnostic| { diagnostic.rule == "avoid-ignoring-return-values" }),
            "{before}: {initial:?}"
        );
        state.cross_file_pass();
        for (version, source, expected_warning) in [(2, after, true), (3, before, false)] {
            let snapshots = state.semantic_snapshot_build_count;
            let topologies = state.semantic_topology_build_count;
            let dependent_analyses = state.document(&dependent_uri).unwrap().analyze_count;
            assert!(state.change(&api_uri, source.into(), Some(version)));
            let analyzed = state.analyze_affected_many(std::slice::from_ref(&api_uri));
            assert_eq!(
                analyzed.iter().map(|(uri, _)| uri).collect::<HashSet<_>>(),
                HashSet::from([&api_uri, &dependent_uri]),
                "{source}: {analyzed:?}"
            );
            let diagnostics = &analyzed
                .iter()
                .find(|(uri, _)| uri == &dependent_uri)
                .unwrap()
                .1;
            assert_eq!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| { diagnostic.rule == "avoid-ignoring-return-values" })
                    .count(),
                usize::from(expected_warning),
                "{source}: {diagnostics:?}"
            );
            assert_eq!(
                state.document(&dependent_uri).unwrap().analyze_count,
                dependent_analyses + 1
            );
            state.cross_file_pass();
            assert!(state.semantic_snapshot_build_count > snapshots, "{source}");
            assert!(state.semantic_topology_build_count > topologies, "{source}");
        }
    }
}

#[test]
fn declaration_updates_rebuild_only_affected_libraries_in_both_views() {
    for semantic in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(&config, "{}").unwrap();
        for (name, source) in [
            ("api.dart", "void api() {}"),
            ("main.dart", "import 'api.dart'; void main() { api(); }"),
            ("unrelated.dart", "class Unrelated {}"),
        ] {
            std::fs::write(directory.path().join(name), source).unwrap();
        }
        let uri = format!("file://{}", directory.path().join("api.dart").display());
        let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
        state.resolve_semantics = semantic;
        state.open(&uri, "void api() {}".into(), Some(1));
        let rebuilds = if semantic {
            state.ensure_semantic_snapshot();
            state.semantic_library_rebuild_count
        } else {
            let (files, _) = state.collect_project_files(ProjectView::CrossFile);
            state.ensure_cross_file_indexes(&files);
            state.cross_file_library_rebuild_count
        };
        state.change(&uri, "int api() => 1;".into(), Some(2));
        let after = if semantic {
            state.ensure_semantic_snapshot();
            state.semantic_library_rebuild_count
        } else {
            let (files, _) = state.collect_project_files(ProjectView::CrossFile);
            state.ensure_cross_file_indexes(&files);
            state.cross_file_library_rebuild_count
        };
        assert_eq!(after - rebuilds, 2, "semantic={semantic}");
    }
}

#[test]
fn import_and_part_edits_rebuild_only_affected_libraries_in_both_views() {
    for semantic in [false, true] {
        for (directive, expected_rebuilds) in [("import", 1), ("part", 2)] {
            let directory = tempfile::tempdir().unwrap();
            let config = directory.path().join("falcon.json");
            std::fs::write(&config, "{}").unwrap();
            let before = format!("import 'external.dart'; {directive} 'a.dart'; void main() {{}}");
            for (name, source) in [
                ("main.dart", before.as_str()),
                ("a.dart", "class A {}"),
                ("b.dart", "class B {}"),
                ("unrelated.dart", "class Unrelated {}"),
                ("external.dart", "class External {}"),
            ] {
                std::fs::write(directory.path().join(name), source).unwrap();
            }
            let uri = format!("file://{}", directory.path().join("main.dart").display());
            let mut state =
                LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
            state.resolve_semantics = semantic;
            let external = directory.path().join("external.dart");
            state
                .config
                .files
                .includes
                .extend_excludes(&[external.to_string_lossy().into_owned()]);
            state.open(&uri, before, Some(1));
            if semantic {
                state.ensure_semantic_snapshot();
            } else {
                let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
                assert!(complete);
                state.ensure_cross_file_indexes(&files);
            }
            for (version, target) in [(2, "b.dart"), (3, "a.dart")] {
                let rebuilds = if semantic {
                    state.semantic_library_rebuild_count
                } else {
                    state.cross_file_library_rebuild_count
                };
                let external_identity = (!semantic).then(|| {
                    Arc::clone(
                        &state
                            .cross_file_external_cache
                            .as_ref()
                            .unwrap()
                            .indexes
                            .as_ref()
                            .unwrap()
                            .identities,
                    )
                });
                assert!(state.change(
                    &uri,
                    format!("import 'external.dart'; {directive} '{target}'; void main() {{}}"),
                    Some(version),
                ));
                let after = if semantic {
                    state.ensure_semantic_snapshot();
                    state.semantic_library_rebuild_count
                } else {
                    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
                    assert!(complete);
                    state.ensure_cross_file_indexes(&files);
                    assert!(Arc::ptr_eq(
                        external_identity.as_ref().unwrap(),
                        &state
                            .cross_file_external_cache
                            .as_ref()
                            .unwrap()
                            .indexes
                            .as_ref()
                            .unwrap()
                            .identities,
                    ));
                    state.cross_file_library_rebuild_count
                };
                assert_eq!(
                    after - rebuilds,
                    expected_rebuilds,
                    "semantic={semantic}, directive={directive}, target={target}",
                );
            }
        }
    }
}

#[test]
fn coalesced_part_regrouping_keeps_old_and_new_dependents() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let sources = [
        ("a.dart", "part 'shared.dart';"),
        ("b.dart", "class B {}"),
        ("shared.dart", "part of 'a.dart'; class Shared {}"),
        ("uses_a.dart", "import 'a.dart'; void use(Shared value) {}"),
        ("uses_b.dart", "import 'b.dart'; void use(B value) {}"),
    ];
    let uris: Vec<_> = sources
        .iter()
        .map(|(name, source)| {
            let path = directory.path().join(name);
            std::fs::write(&path, source).unwrap();
            format!("file://{}", path.display())
        })
        .collect();
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    for (uri, (_, source)) in uris.iter().zip(sources) {
        state.open(uri, source.into(), Some(1));
    }
    state.change(&uris[0], "class A {}".into(), Some(2));
    state.change(&uris[1], "part 'shared.dart'; class B {}".into(), Some(2));
    state.change(
        &uris[2],
        "part of 'b.dart'; class Shared {}".into(),
        Some(2),
    );
    let affected = state.affected_open_uris(&uris[2]);
    assert!(affected.contains(&uris[3]), "{affected:?}");
    assert!(affected.contains(&uris[4]), "{affected:?}");
}

#[test]
fn closing_coalesced_declaration_edit_restores_disk_before_incremental_update() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let path = directory.path().join("api.dart");
    std::fs::write(&path, "void api() {}").unwrap();
    std::fs::write(
        directory.path().join("main.dart"),
        "import 'api.dart'; void main() { api(); }",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("unrelated.dart"),
        "class Unrelated {}",
    )
    .unwrap();
    let uri = format!("file://{}", path.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    state.open(&uri, "void api() {}".into(), Some(1));
    let rebuilds = state.semantic_library_rebuild_count;
    state.change(&uri, "int api() => 1;".into(), Some(2));
    state.change(&uri, "String api() => 'value';".into(), Some(3));
    state.close(&uri);
    state.ensure_semantic_snapshot();
    assert_eq!(state.semantic_library_rebuild_count, rebuilds);
    let snapshot = state.semantic_snapshot.as_ref().unwrap();
    assert_eq!(
        snapshot.files[snapshot.by_path[&path]].source(),
        "void api() {}"
    );
}

#[test]
fn coalesced_declaration_and_directive_edits_match_fresh_semantics() {
    use falcon_analyze::SemanticModel;

    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}}}"#).unwrap();
    let sources = [
        ("api.dart", "void api() {}"),
        ("barrel.dart", "export 'api.dart';"),
        ("main.dart", "import 'barrel.dart'; void main() { api(); }"),
        ("part.dart", "part of 'api.dart'; class Helper {}"),
        ("unrelated.dart", "class Unrelated {}"),
    ];
    let mut state = LspState::new_from(
        Some(config.clone()),
        |_| None,
        directory.path().to_path_buf(),
    );
    state.resolve_semantics = true;
    let uris: Vec<_> = sources
        .iter()
        .map(|(name, source)| {
            let path = directory.path().join(name);
            std::fs::write(&path, source).unwrap();
            format!("file://{}", path.display())
        })
        .collect();
    for (uri, (_, source)) in uris.iter().zip(sources) {
        state.open(uri, source.into(), Some(1));
    }
    let (files, _) = state.collect_project_files(ProjectView::CrossFile);
    state.ensure_cross_file_indexes(&files);
    for edits in [
        vec![(0, "int api() => 1;"), (1, "export 'api.dart' show api;")],
        vec![
            (0, "part 'part.dart'; int api() => 1;"),
            (2, "import 'api.dart'; void main() { api(); }"),
        ],
        vec![(0, "void api() {}"), (3, "class Helper {}")],
    ] {
        for (index, source) in edits {
            state.change(&uris[index], source.into(), Some(2));
        }
        state.analyze_affected_many(&uris);
        let mut fresh = LspState::new_from(
            Some(config.clone()),
            |_| None,
            directory.path().to_path_buf(),
        );
        fresh.resolve_semantics = true;
        for uri in &uris {
            fresh.open(uri, state.documents[uri].text().into(), Some(2));
        }
        for uri in &uris {
            assert_eq!(
                format!("{:?}", state.analyze(uri)),
                format!("{:?}", fresh.analyze(uri))
            );
        }
        assert_semantic_snapshot_values_match(&state, &fresh);
        // Exercise the independently retained cache too, rather than borrowing
        // the current semantic snapshot through cross_file_pass.
        let (files, _) = state.collect_project_files(ProjectView::CrossFile);
        state.ensure_cross_file_indexes(&files);
        let (fresh_files, _) = fresh.collect_project_files(ProjectView::CrossFile);
        fresh.ensure_cross_file_indexes(&fresh_files);
        let cache = state.cross_file_index_cache.as_ref().unwrap();
        let rebuilt = fresh.cross_file_index_cache.as_ref().unwrap();
        for file in &cache.files {
            let fresh_file = rebuilt
                .files
                .iter()
                .find(|candidate| candidate.path == file.path)
                .unwrap();
            let cached = SemanticModel::new(
                &file.path,
                &file.parsed,
                &cache.indexes.identities,
                Some(&cache.indexes.types),
            );
            let model = SemanticModel::new(
                &file.path,
                &fresh_file.parsed,
                &rebuilt.indexes.identities,
                Some(&rebuilt.indexes.types),
            );
            for name in ["api", "Helper", "Unrelated"] {
                assert_eq!(cached.resolve_value(name), model.resolve_value(name));
            }
        }
    }
}

fn assert_semantic_snapshot_values_match(state: &LspState, fresh: &LspState) {
    use falcon_analyze::SemanticModel;

    let snapshot = state.semantic_snapshot.as_ref().unwrap();
    let rebuilt = fresh.semantic_snapshot.as_ref().unwrap();
    for file in &snapshot.files {
        let cached = SemanticModel::new(
            &file.path,
            &file.parsed,
            &snapshot.indexes.identities,
            Some(&snapshot.indexes.types),
        );
        let fresh_file = &rebuilt.files[rebuilt.by_path[&file.path]];
        let model = SemanticModel::new(
            &file.path,
            &fresh_file.parsed,
            &rebuilt.indexes.identities,
            Some(&rebuilt.indexes.types),
        );
        for name in ["api", "Helper", "Unrelated"] {
            assert_eq!(cached.resolve_value(name), model.resolve_value(name));
        }
    }
}

#[test]
fn body_only_edits_refresh_loop_mutation_facts_without_rebuilding_indexes() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let path = directory.path().join("main.dart");
    let source = |body: &str| {
        format!(
            "class Value {{ int work() => 1; }} \
             void main(dynamic value, bool repeat) {{ if (value is Value) {{ {body} }} }}"
        )
    };
    let before = source("while (repeat) { value.work(); }");
    std::fs::write(&path, &before).unwrap();
    let uri = format!("file://{}", path.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    let initial = state.open(&uri, before.clone(), Some(1));
    assert_eq!(
        initial
            .iter()
            .filter(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
            .count(),
        1,
        "{initial:?}"
    );
    let snapshots = state.semantic_snapshot_build_count;
    let topologies = state.semantic_topology_build_count;
    let external_indexes = state.external_index_build_count;
    for (version, changed, expected_warnings) in [
        (
            2,
            source("while (repeat) { value.work(); value = null; }"),
            0,
        ),
        (
            3,
            source("/* shifted loop */ while (repeat) { value.work(); value = null; }"),
            0,
        ),
        (4, before, 1),
    ] {
        assert!(state.change(&uri, changed, Some(version)));
        let analyzed = state.analyze_affected_many(std::slice::from_ref(&uri));
        assert_eq!(analyzed.len(), 1, "{analyzed:?}");
        let diagnostics = &analyzed[0].1;
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
                .count(),
            expected_warnings,
            "{diagnostics:?}"
        );
        assert_eq!(state.semantic_snapshot_build_count, snapshots);
        assert_eq!(state.semantic_topology_build_count, topologies);
        assert_eq!(state.external_index_build_count, external_indexes);
    }
}

#[test]
fn cross_file_only_indexes_reuse_explicit_bodies_but_rebuild_changed_return_types() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let path = directory.path().join("main.dart");
    let before = "int api() => 1; inferred() => 1; void main() { api(); inferred(); }";
    std::fs::write(&path, before).unwrap();
    let uri = format!("file://{}", path.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = false;
    state.cross_file_resolve_semantics = true;
    state.open(&uri, before.into(), Some(1));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    state.ensure_cross_file_indexes(&files);
    assert_eq!(state.cross_file_index_build_count, 1);
    state.ensure_cross_file_indexes(&files);
    assert_eq!(state.cross_file_index_build_count, 1);
    for (version, source, expected_builds) in [
        (
            2,
            "int api() => 2; inferred() => 1; void main() { api(); inferred(); }",
            1,
        ),
        (
            3,
            "String api() => 'value'; inferred() => 1; void main() { api(); inferred(); }",
            2,
        ),
        (
            4,
            "String api() => 'value'; inferred() => 'changed'; void main() { api(); inferred(); }",
            3,
        ),
    ] {
        assert!(state.change(&uri, source.into(), Some(version)));
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        state.ensure_cross_file_indexes(&files);
        assert_eq!(
            state.cross_file_index_build_count, expected_builds,
            "{source}"
        );
        let cached = state.cross_file_index_cache.as_ref().unwrap();
        let cached_file = cached.files.iter().find(|file| file.path == path).unwrap();
        assert!(Arc::ptr_eq(
            &cached_file.parsed,
            &state.documents[&uri].parsed
        ));
        assert!(state.semantic_snapshot.is_none());
        assert_eq!(state.semantic_snapshot_build_count, 0);
        assert_eq!(state.semantic_topology_build_count, 0);
    }
}

#[test]
fn atomic_source_replacement_reuses_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let main = directory.path().join("main.dart");
    let support = directory.path().join("support.dart");
    std::fs::write(&main, "void main() {}").unwrap();
    std::fs::write(&support, "class Before {}").unwrap();
    let uri = format!("file://{}", main.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    state.open(&uri, "void main() {}".into(), Some(1));
    let parses = state.disk_parse_count;
    let walks = state.workspace_walk_count;
    let temporary = directory.path().join("support.dart.tmp");
    let changed = "class AfterReplacement {}";
    std::fs::write(&temporary, changed).unwrap();
    std::fs::rename(&temporary, &support).unwrap();
    state.watched_files_changed(std::slice::from_ref(&support));
    state.change(&uri, "void main() { print(1); }".into(), Some(2));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        changed
    );
    assert_eq!(state.disk_parse_count, parses + 1);
    assert_eq!(state.workspace_walk_count, walks);
}

#[test]
fn unsupported_directory_entries_reuse_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let git = directory.path().join(".git");
    std::fs::create_dir(&git).unwrap();
    let main = directory.path().join("main.dart");
    std::fs::write(&main, "void main() {}").unwrap();
    let uri = format!("file://{}", main.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    state.open(&uri, "void main() {}".into(), Some(1));
    let walks = state.workspace_walk_count;
    let parses = state.disk_parse_count;
    for (index, temporary) in [
        directory.path().join("editor.tmp"),
        git.join("index.lock"),
        git.join("index"),
    ]
    .into_iter()
    .enumerate()
    {
        let parent = temporary.parent().unwrap();
        for remove in [false, true] {
            let modified = std::fs::metadata(parent).unwrap().modified().unwrap();
            if remove {
                std::fs::remove_file(&temporary).unwrap();
            } else {
                std::fs::write(&temporary, "background update").unwrap();
            }
            std::fs::File::open(parent)
                .unwrap()
                .set_modified(modified + std::time::Duration::from_secs(2))
                .unwrap();
            let version =
                2 + i32::try_from(index * 2 + usize::from(remove)).expect("test version fits i32");
            state.change(
                &uri,
                format!("void main() {{ print({version}); }}"),
                Some(version),
            );
            let (_, complete) = state.collect_project_files(ProjectView::CrossFile);
            assert!(complete);
            assert_eq!(state.workspace_walk_count, walks);
            assert_eq!(state.disk_parse_count, parses);
        }
    }
    let modified = std::fs::metadata(&git).unwrap().modified().unwrap();
    std::fs::write(git.join("index"), "old index").unwrap();
    std::fs::write(git.join("index.lock"), "new index").unwrap();
    std::fs::rename(git.join("index.lock"), git.join("index")).unwrap();
    std::fs::File::open(&git)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    state.change(&uri, "void main() { print(10); }".into(), Some(10));
    state.collect_project_files(ProjectView::CrossFile);
    assert_eq!(state.workspace_walk_count, walks);
    assert_eq!(state.disk_parse_count, parses);
}

#[test]
fn new_and_renamed_directories_refresh_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    std::fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.collect_project_files(ProjectView::CrossFile);
    let walks = state.workspace_walk_count;
    let nested = directory.path().join("new/nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("added.dart"), "class Added {}").unwrap();
    state.watched_files_changed(&[directory.path().join("new")]);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert!(
        files
            .iter()
            .any(|file| file.path == nested.join("added.dart"))
    );
    assert_eq!(state.workspace_walk_count, walks + 1);
    std::fs::rename(
        directory.path().join("new"),
        directory.path().join("renamed"),
    )
    .unwrap();
    state.watched_files_changed(&[
        directory.path().join("new"),
        directory.path().join("renamed"),
    ]);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert!(
        files
            .iter()
            .any(|file| file.path == directory.path().join("renamed/nested/added.dart"))
    );
    assert!(
        !files
            .iter()
            .any(|file| file.path.starts_with(directory.path().join("new")))
    );
    assert_eq!(state.workspace_walk_count, walks + 2);
}

#[test]
fn watched_disk_changes_refresh_cached_cross_file_input() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let main = directory.path().join("main.dart");
    let support = directory.path().join("support.dart");
    std::fs::write(&main, "void main() {}").unwrap();
    std::fs::write(&support, "class Before {}").unwrap();
    let uri = format!("file://{}", main.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = true;
    state.open(&uri, "void main() {}".into(), Some(1));
    let parses = state.disk_parse_count;
    let walks = state.workspace_walk_count;
    let modified = std::fs::metadata(&support).unwrap().modified().unwrap();
    let changed = "class After_ {}";
    assert_eq!(changed.len(), "class Before {}".len());
    std::fs::write(&support, changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&support)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&support).unwrap().modified().unwrap(),
        modified
    );
    state.watched_files_changed(std::slice::from_ref(&support));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    let file = files.iter().find(|file| file.path == support).unwrap();
    assert_eq!(file.source(), changed);
    assert_eq!(state.disk_parse_count, parses + 1);
    assert_eq!(state.workspace_walk_count, walks);
    let added = directory.path().join("added.dart");
    std::fs::write(&added, "class Added {}").unwrap();
    state.watched_files_changed(std::slice::from_ref(&added));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert!(files.iter().any(|file| file.path == added));
}

#[test]
fn changed_disk_mtime_reuses_unchanged_parsed_source() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let support = directory.path().join("support.dart");
    let source = "class Support {}";
    std::fs::write(&support, source).unwrap();
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    let (before, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    let before = before.iter().find(|file| file.path == support).unwrap();
    let parses = state.disk_parse_count;
    assert_eq!(parses, 1);
    let modified = std::fs::metadata(&support).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(&support)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(2))
        .unwrap();
    assert_ne!(
        std::fs::metadata(&support).unwrap().modified().unwrap(),
        modified
    );

    let (after, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    let after = after.iter().find(|file| file.path == support).unwrap();
    assert_eq!(after.source(), source);
    assert!(Arc::ptr_eq(&before.parsed, &after.parsed));
    assert_eq!(state.disk_parse_count, parses);
}

#[test]
fn save_invalidates_cached_disk_source_with_unchanged_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let support = directory.path().join("support.dart");
    let original = "class Before {}";
    let changed = "class After_ {}";
    assert_eq!(original.len(), changed.len());
    std::fs::write(&support, original).unwrap();
    let uri = format!("file://{}", support.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        original
    );
    let parses = state.disk_parse_count;
    assert_eq!(parses, 1);
    let modified = std::fs::metadata(&support).unwrap().modified().unwrap();
    std::fs::write(&support, changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&support)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&support).unwrap().modified().unwrap(),
        modified
    );

    // Keep the document closed so buffer precedence and close invalidation
    // cannot conceal whether the save event refreshes the disk input.
    state.save(&uri, None);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        changed
    );
    assert_eq!(state.disk_parse_count, parses + 1);
}

#[test]
fn close_refreshes_cached_disk_source_with_unchanged_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let support = directory.path().join("support.dart");
    let original = "class Before {}";
    let changed = "class After_ {}";
    assert_eq!(original.len(), changed.len());
    std::fs::write(&support, original).unwrap();
    let uri = format!("file://{}", support.display());
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        original
    );
    let parses = state.disk_parse_count;
    assert_eq!(parses, 1);
    state.open(&uri, "class Buffered {}".into(), Some(1));
    let modified = std::fs::metadata(&support).unwrap().modified().unwrap();
    std::fs::write(&support, changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&support)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&support).unwrap().modified().unwrap(),
        modified
    );
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        "class Buffered {}"
    );
    assert_eq!(state.disk_parse_count, parses);

    state.close(&uri);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == support)
            .unwrap()
            .source(),
        changed
    );
    assert_eq!(state.disk_parse_count, parses + 1);
}

#[test]
fn ancestor_manifest_cross_file_findings_refresh_after_disk_change() {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    let workspace = root.join("tool");
    std::fs::create_dir(&workspace).unwrap();
    let manifest = root.join("pubspec.yaml");
    std::fs::write(
        &manifest,
        "name: ancestor_fixture\nhomepage: http://example.com\n",
    )
    .unwrap();
    let config = workspace.join("falcon.json");
    std::fs::write(
        &config,
        r#"{
          "linter": { "enabled": false },
          "cross-file": {
            "rules": {
              "recommended": false,
              "security": { "secure-pubspec-urls": "error" }
            }
          }
        }"#,
    )
    .unwrap();
    let dart = workspace.join("main.dart");
    let source = "void main() {}\n";
    std::fs::write(&dart, source).unwrap();
    let uri = format!("file://{}", dart.display());
    let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
    assert!(state.open(&uri, source.to_owned(), Some(1)).is_empty());
    assert_eq!(
        state.take_config_messages(),
        [] as [lsp_types::ShowMessageParams; 0]
    );
    let findings = state.cross_file_diagnostics();
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let diagnostics = &findings[manifest.to_string_lossy().as_ref()];
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.rule, "secure-pubspec-urls");
    assert_eq!(diagnostic.severity, falcon_diagnostics::Severity::Error);
    assert_eq!(diagnostic.file_path, manifest.to_string_lossy());
    assert_eq!(
        diagnostic.message,
        "The `http` protocol shouldn't be used because it isn't secure."
    );
    assert_eq!(diagnostic.line, 2);
    assert!(diagnostic.col > 0);
    // The real server opens Dart documents only; manifest findings remain in
    // the project map instead of being published as artificial Dart buffers.
    assert!(state.cross_file_pass().is_empty());

    std::fs::write(
        &manifest,
        "name: ancestor_fixture\nhomepage: https://example.com\n",
    )
    .unwrap();
    let refreshed = state.watched_files_changed(std::slice::from_ref(&manifest));
    assert_eq!(refreshed.len(), 1, "{refreshed:#?}");
    assert_eq!(refreshed[0].0, uri);
    assert!(refreshed[0].1.is_empty());
    assert!(state.cross_file_diagnostics().is_empty());
}

#[cfg(test)]
mod source_size_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct CountCrossRuns(Arc<AtomicUsize>);
    impl falcon_analyze::CrossFileRule for CountCrossRuns {
        fn prepare_project(
            &self,
            _environment: &AnalysisEnvironment,
            _files: &[ProjectFile],
            _config: &FalconConfig,
            _indexes: Option<&SemanticIndexes>,
        ) -> Vec<Diagnostic> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }
    }

    #[test]
    fn parser_worker_panics_publish_worker_diagnostics_not_syntax_errors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.dart");
        let uri = format!("file://{}", path.display());
        let config = directory.path().join("falcon.json");
        std::fs::write(&config, "{}").unwrap();
        std::fs::write(&path, "void main() {}").unwrap();
        let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
        let failure_path = canonical_or_lexical(&path);
        let assert_failure = |diagnostics: &[Diagnostic]| {
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                diagnostics[0].rule,
                falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
            );
            assert!(diagnostics[0].message.contains("project parser regression"));
        };
        state.parser_failure_path = Some(failure_path.clone());
        assert_failure(&state.open(&uri, "void main() {}".into(), Some(1)));
        assert!(state.document(&uri).unwrap().source_rejected);
        state.parser_failure_path = None;
        assert!(state.change(&uri, "void main() {}".into(), Some(2)));
        assert!(!state.document(&uri).unwrap().source_rejected);
        state.parser_failure_path = Some(failure_path);
        assert!(state.change(&uri, "void main() {}".into(), Some(3)));
        assert_failure(&state.analyze(&uri));
        assert!(state.document(&uri).unwrap().source_rejected);
        state.parser_failure_path = None;
        assert!(state.change(&uri, "void main() {}".into(), Some(4)));
        assert!(!state.document(&uri).unwrap().source_rejected);
        assert!(
            state.analyze(&uri).iter().all(|diagnostic| diagnostic.rule
                != falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID)
        );
    }

    #[test]
    fn project_parse_failures_publish_once_skip_cross_rules_and_recover_on_disk_events() {
        for companion in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let app = directory.path().join("app");
            let dep = directory.path().join("dep/lib");
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(&dep).unwrap();
            std::fs::write(
                app.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#,
            )
            .unwrap();
            let config = app.join("falcon.json");
            std::fs::write(&config, "{}").unwrap();
            let mut state = LspState::new_from(Some(config), |_| None, app.clone());
            state.resolve_semantics = true;
            let failed = if companion {
                let defining = dep.join("leaf.dart");
                std::fs::write(
                    &defining,
                    "library leaf; part 'leaf_part.dart'; class Leaf {}",
                )
                .unwrap();
                let part = dep.join("leaf_part.dart");
                std::fs::write(&part, "part of leaf; class OnDisk {}").unwrap();
                state.open(
                    &format!("file://{}", part.display()),
                    "part of leaf; class Buffered {}".into(),
                    Some(1),
                );
                defining
            } else {
                let failed = app.join("failed.dart");
                std::fs::write(&failed, "class DiskFile {}").unwrap();
                state.open(
                    &format!("file://{}", app.join("healthy.dart").display()),
                    "void healthy() {}".into(),
                    Some(1),
                );
                failed
            };
            state.open(
                &format!("file://{}", app.join("main.dart").display()),
                if companion {
                    "import 'package:dep/leaf.dart';".into()
                } else {
                    "void main() {}".into()
                },
                Some(1),
            );
            let failed = canonical_or_lexical(&failed);
            state.parser_failure_path = Some(failed.clone());
            let source = std::fs::read_to_string(&failed).unwrap();
            std::fs::write(&failed, format!("{source}\n")).unwrap();
            let runs = Arc::new(AtomicUsize::new(0));
            state.cross_file_rules.register(
                "count-cross-runs",
                Box::new(CountCrossRuns(Arc::clone(&runs))),
            );
            state.watched_files_changed(std::slice::from_ref(&failed));
            let published = state.cross_file_pass();
            assert_eq!(published.len(), 2);
            for (_, diagnostics) in published {
                let failures: Vec<_> = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.rule == falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
                    })
                    .collect();
                assert_eq!(failures.len(), 1, "{diagnostics:?}");
                assert!(failures[0].message.contains("project parser regression"));
            }
            assert_eq!(runs.load(Ordering::SeqCst), 0);
            assert!(state.semantic_snapshot.is_none());
            assert!(state.semantic_snapshot_error.is_some());
            assert!(state.watched_local_sources.contains(&failed));
            assert!(state.watched_local_paths.contains(&watched_path(&failed)));
            state.parser_failure_path = None;
            let recovered = state.watched_files_changed(&[failed]);
            assert_eq!(recovered.len(), 2);
            assert!(
                recovered
                    .iter()
                    .all(|(_, diagnostics)| diagnostics.iter().all(|diagnostic| {
                        diagnostic.rule != falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
                    }))
            );
            assert!(state.semantic_snapshot_error.is_none());
            assert!(state.semantic_snapshot.is_some());
        }
    }

    #[test]
    fn build_panics_publish_once_and_recover_on_dependency_events() {
        for cross_file in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let app = directory.path().join("app");
            let dep = directory.path().join("dep/lib");
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(&dep).unwrap();
            std::fs::write(
                app.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#,
            )
            .unwrap();
            let dependency = dep.join("dep.dart");
            std::fs::write(&dependency, "class Dep {}").unwrap();
            let dependency = canonical_or_lexical(&dependency);
            let config = app.join("falcon.json");
            std::fs::write(&config, "{}").unwrap();
            let mut state = LspState::new_from(Some(config), |_| None, app.clone());
            // Without per-file semantic rules, only the cross-file view loads dependencies.
            state.resolve_semantics = !cross_file;
            state.cross_file_resolve_semantics = cross_file;
            let runs = Arc::new(AtomicUsize::new(0));
            state.cross_file_rules.register(
                "count-cross-runs",
                Box::new(CountCrossRuns(Arc::clone(&runs))),
            );
            state.build_panic = true;
            let uri = format!("file://{}", app.join("main.dart").display());
            state.open(&uri, "import 'package:dep/dep.dart';".into(), Some(1));
            let published = state.cross_file_pass();
            assert_eq!(published.len(), 1);
            let failures: Vec<_> = published[0]
                .1
                .iter()
                .filter(|diagnostic| {
                    diagnostic.rule == falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
                })
                .collect();
            assert_eq!(failures.len(), 1, "{:?}", published[0].1);
            assert!(failures[0].message.contains("semantic build regression"));
            assert_eq!(runs.load(Ordering::SeqCst), 0);
            assert!(
                state
                    .environment
                    .watched_dependency_roots()
                    .any(|root| dependency.starts_with(root))
            );

            // The failure stays recorded instead of rebuilding on every pass.
            state.build_panic = false;
            state.cross_file_pass();
            assert_eq!(runs.load(Ordering::SeqCst), 0);
            assert!(state.semantic_snapshot.is_none());
            assert!(state.cross_file_index_cache.is_none());

            let recovered = state.watched_files_changed(std::slice::from_ref(&dependency));
            assert_eq!(recovered.len(), 1);
            assert!(recovered[0].1.iter().all(|diagnostic| {
                diagnostic.rule != falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
            }));
            assert!(state.semantic_snapshot_error.is_none());
            state.cross_file_pass();
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            assert_eq!(state.semantic_snapshot.is_some(), !cross_file);
            assert_eq!(state.cross_file_index_cache.is_some(), cross_file);
        }
    }

    #[test]
    fn ordinary_disk_read_errors_keep_project_collection_complete() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("main.dart");
        std::fs::write(&valid, "void main() {}").unwrap();
        std::fs::write(directory.path().join("invalid.dart"), [0xff]).unwrap();
        std::fs::write(directory.path().join("pubspec.yaml"), [0xff]).unwrap();
        std::fs::create_dir(directory.path().join("directory.dart")).unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(&config, "{}").unwrap();
        let mut state = LspState::new(Some(config));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, canonical_or_lexical(&valid));
    }

    #[test]
    fn ordinary_walk_errors_keep_project_collection_complete() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = LspState::new(None);
        state.initialize_workspace_roots(vec![directory.path().join("missing")]);
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert!(files.is_empty());
        let root = directory.path().join("missing");
        std::fs::create_dir(&root).unwrap();
        let created = root.join("created.dart");
        std::fs::write(&created, "class Created {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&created));
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert!(files.iter().any(|file| file.path == created));
    }

    #[test]
    fn rejected_buffer_diagnostic_survives_refresh_and_recovers_on_change() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
        let uri = format!("file://{}", directory.path().join("main.dart").display());
        let mut state = LspState::new(None);
        state.initialize_workspace_roots(vec![directory.path().to_path_buf()]);
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state.open(&uri, String::new(), Some(1));
        let doc = state.documents.get_mut(&uri).unwrap();
        doc.source_rejected = true;
        doc.has_parse_errors = true;
        doc.syntax_diagnostics = vec![
            falcon_analyze::source_size_diagnostic(&doc.path, u64::from(u32::MAX) + 1).unwrap(),
        ];
        let parse_count = doc.parse_count;
        let diagnostics = state.analyze(&uri);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "syntax-error")
        );
        assert_eq!(state.document(&uri).unwrap().parse_count, parse_count);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            state
                .collect_project_files(ProjectView::Semantic)
                .0
                .is_empty()
        );
        assert!(
            state
                .collect_project_files(ProjectView::CrossFile)
                .0
                .is_empty()
        );
        assert!(state.cross_file_diagnostics().is_empty());

        let healthy = canonical_or_lexical(&directory.path().join("healthy.dart"));
        let healthy_uri = format!("file://{}", healthy.display());
        state.resolve_semantics = true;
        state.open(&healthy_uri, "void healthy() {}".into(), Some(1));
        state.ensure_semantic_snapshot();
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.files[0].path, healthy);
        assert!(state.semantic_snapshot_error.is_none());

        assert!(state.change(&uri, "void main() {}".into(), Some(2)));
        assert!(!state.document(&uri).unwrap().source_rejected);
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "syntax-error")
        );
    }

    #[test]
    fn supported_buffer_text_is_not_modified() {
        let mut text = "// 🦅\nvoid main() {}".to_string();
        let original = text.clone();
        assert!(reject_oversized_buffer(Path::new("main.dart"), &mut text).is_none());
        assert_eq!(text, original);
    }

    #[test]
    fn oversized_disk_sources_clear_cross_file_diagnostics_and_recover() {
        for name in ["oversized.dart", "pubspec.yaml"] {
            let directory = tempfile::tempdir().unwrap();
            let config = directory.path().join("falcon.json");
            std::fs::write(
                &config,
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
            )
            .unwrap();
            let lib = directory.path().join("lib");
            std::fs::create_dir(&lib).unwrap();
            std::fs::write(lib.join("main.dart"), "void main() {}").unwrap();
            let orphan = lib.join("orphan.dart");
            std::fs::write(&orphan, "class Orphan {}").unwrap();
            let uri = format!("file://{}", orphan.display());
            let mut state = LspState::new(Some(config));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            state.open(&uri, "class Orphan {}".into(), Some(1));
            assert!(state.cross_file_pass().iter().any(|(_, diagnostics)| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unused-files")
            }));

            let rejected = directory.path().join(name);
            std::fs::File::create(&rejected)
                .unwrap()
                .set_len(u64::from(u32::MAX) + 1)
                .unwrap();
            state.watched_files_changed(std::slice::from_ref(&rejected));
            let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
            assert!(!complete);
            assert!(files.iter().all(|file| file.path != rejected));
            assert!(state.cross_file_diagnostics().is_empty());
            let published = state.cross_file_pass();
            let (_, diagnostics) = published.iter().find(|(key, _)| key == &uri).unwrap();
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.rule != "unused-files")
            );
            state.resolve_semantics = true;
            state.invalidate_semantic_snapshot();
            state.ensure_semantic_snapshot();
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert!(
                snapshot
                    .files
                    .iter()
                    .any(|file| file.path == canonical_or_lexical(&orphan))
            );
            assert!(snapshot.files.iter().all(|file| file.path != rejected));
            assert!(state.semantic_snapshot_error.is_none());

            std::fs::remove_file(&rejected).unwrap();
            state.watched_files_changed(std::slice::from_ref(&rejected));
            assert!(state.cross_file_diagnostics().values().any(|diagnostics| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unused-files")
            }));
        }
    }
}
#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn canonicalizing_loader_preserves_legacy_rule_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "linter": {"rules": {"recommended": false, "complexity": {"avoid_nested_if": "error"}}},
            "cross-file": {"enabled": false}
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path), |_| None);
        assert!(state.config.linter.rules.groups["complexity"].contains_key("avoid-nested-if"));
        assert!(!state.config.linter.rules.groups["complexity"].contains_key("avoid_nested_if"));
        assert!(
            state
                .take_config_messages()
                .iter()
                .any(|message| message.typ == MessageType::WARNING
                    && message.message.contains("avoid_nested_if"))
        );
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        let diagnostics = state.open(
            &uri,
            "void f(bool a, bool b, bool c) { if (a) { if (b) { if (c) { print(a); } } } }\n"
                .to_string(),
            Some(1),
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-nested-if"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error)
        );
    }

    const SOURCE: &str = "void f() { dynamic value = 1; print(value); }\n";

    #[test]
    fn relative_filters_and_overrides_match_absolute_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "files": {"includes": ["lib/**", "!lib/generated/**"]},
            "linter": {"rules": {"recommended": false, "suspicious": {"avoid-dynamic": "warn"}}},
            "cross-file": {"enabled": false},
            "overrides": [{
                "includes": ["lib/quiet/*.dart"],
                "linter": {"rules": {"suspicious": {"avoid-dynamic": "off"}}}
            }]
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path), |_| None);
        for (relative, expected) in [
            ("lib/main.dart", true),
            ("lib/quiet/main.dart", false),
            ("lib/quiet/deep/main.dart", true),
            ("lib/generated/main.dart", false),
            ("test/main.dart", false),
        ] {
            let uri = format!("file://{}", dir.path().join(relative).display());
            let diagnostics = state.open(&uri, SOURCE.to_string(), Some(1));
            assert_eq!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-dynamic"),
                expected,
                "{relative}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn discovered_config_root_drives_workspace_collection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("falcon.json"), "{}").unwrap();
        let main = dir.path().join("main.dart");
        std::fs::write(&main, "void main() {}\n").unwrap();
        let mut state = LspState::new_from(None, |_| None, dir.path().join("nested"));
        assert_eq!(state.workspace_root, canonical_or_lexical(dir.path()));
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert!(
            files
                .iter()
                .any(|file| file.path == canonical_or_lexical(&main))
        );
    }

    #[test]
    fn excluded_subtrees_do_not_make_workspace_collection_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{
            "files":{"includes":["!build/**"]},
            "linter":{"rules":{"recommended":false}},
            "cross-file":{"enabled":false}
        }"#,
        )
        .unwrap();
        let excluded = dir.path().join("build/deep");
        std::fs::create_dir_all(&excluded).unwrap();
        for name in ["oversized.dart", "pubspec.yaml"] {
            std::fs::File::create(excluded.join(name))
                .unwrap()
                .set_len(u64::from(u32::MAX) + 1)
                .unwrap();
        }
        let main = dir.path().join("main.dart");
        std::fs::write(&main, "void main() {}\n").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        for view in [ProjectView::Semantic, ProjectView::CrossFile] {
            let (files, complete) = state.collect_project_files(view);
            assert!(complete);
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, canonical_or_lexical(&main));
        }
    }

    #[test]
    fn home_fallback_settings_do_not_expand_workspace_to_home() {
        const CHILD: &str = "FALCON_TEST_LSP_HOME_CONFIG_ROOT";
        if std::env::var_os(CHILD).is_none() {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("project");
            std::fs::create_dir_all(&workspace).unwrap();
            std::fs::write(
                dir.path().join(".falcon.json"),
                r#"{
                    "files":{"includes":["lib/**", "!lib/generated/**"]},
                    "linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"warn"}}},
                    "cross-file":{"enabled":false},
                    "overrides":[{
                        "includes":["lib/quiet/**"],
                        "linter":{"rules":{"suspicious":{"avoid-dynamic":"off"}}}
                    }]
                }"#,
            )
            .unwrap();
            #[cfg(not(unix))]
            let home = dir.path().to_path_buf();
            #[cfg(unix)]
            let home = {
                let alias = dir.path().join("home-alias");
                std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
                alias
            };
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "state::tests::config_tests::home_fallback_settings_do_not_expand_workspace_to_home",
                ])
                .env(CHILD, "1")
                .env("HOME", home)
                .current_dir(&workspace)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let workspace = std::env::current_dir().unwrap();
        let home_config = workspace.parent().unwrap().join(".falcon.json");
        let other_workspace = workspace.join("other");
        std::fs::create_dir_all(&other_workspace).unwrap();
        for (config_path, workspace) in [
            (None, workspace.clone()),
            (Some(home_config.clone()), workspace.clone()),
            (Some(PathBuf::from("../.falcon.json")), workspace.clone()),
            (Some(home_config), other_workspace),
        ] {
            let mut state = LspState::new_from(config_path, |_| None, workspace.clone());
            assert_eq!(state.workspace_root, canonical_or_lexical(&workspace));
            assert_eq!(
                state.config.root_dir(),
                Some(state.workspace_root.as_path())
            );
            for (relative, expected) in [
                ("lib/main.dart", true),
                ("lib/quiet/main.dart", false),
                ("lib/generated/main.dart", false),
                ("test/main.dart", false),
            ] {
                let uri = format!("file://{}", workspace.join(relative).display());
                let diagnostics = state.open(&uri, SOURCE.to_string(), Some(1));
                assert_eq!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.rule == "avoid-dynamic"),
                    expected,
                    "{relative}: {diagnostics:?}"
                );
            }
        }

        let local = workspace.join("settings/.falcon.json");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "{}").unwrap();
        let state = LspState::new_from(Some(local), |_| None, workspace.clone());
        assert_eq!(
            state.workspace_root,
            canonical_or_lexical(&workspace.join("settings")),
            "a non-HOME config still determines the workspace root"
        );
    }

    #[test]
    fn invalid_reload_preserves_rules_root_and_open_document_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},
            "cross-file":{"enabled":false}
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let root = state.workspace_root.clone();
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        let before = state.open(&uri, SOURCE.to_string(), Some(1));
        std::fs::write(&path, r#"{"linter":{"enabledd":false}}"#).unwrap();
        assert!(state.reload_config().is_empty());
        assert_eq!(state.workspace_root, root);
        assert!(!state.cross_file_rules_enabled());
        assert_eq!(state.config.linter.rules.recommended, Some(false));
        let after = state.analyze(&uri);
        assert_eq!(before.len(), after.len());
        assert!(
            after
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-dynamic"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error)
        );
        let messages = state.take_config_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].message.contains(&path.display().to_string()));
        assert!(messages[0].message.contains("linter.enabledd"));
        state.reload_config();
        assert_eq!(
            state.take_config_messages(),
            [] as [lsp_types::ShowMessageParams; 0]
        );
    }

    #[test]
    fn nested_config_events_do_not_repeat_unchanged_active_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(&path, r#"{"max_errors":10}"#).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .all(|message| message.message.contains(&path.display().to_string()))
        );
        state.reload_config();
        assert_eq!(
            state.take_config_messages(),
            [] as [lsp_types::ShowMessageParams; 0]
        );
        let nested = dir.path().join("nested/falcon.json");
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(&nested, "{}").unwrap();
        state.watched_files_changed(&[nested]);
        assert_eq!(
            state.take_config_messages(),
            [] as [lsp_types::ShowMessageParams; 0]
        );
    }

    #[test]
    fn config_messages_are_drained_and_reloaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(&path, r#"{"max_errors": 10}"#).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .any(|message| message.typ == MessageType::WARNING
                    && message.message.contains("max_errors"))
        );
        assert_eq!(
            state.take_config_messages(),
            [] as [lsp_types::ShowMessageParams; 0]
        );
        std::fs::write(&path, r#"{"overrides": [{"incldues": ["lib/**"]}]}"#).unwrap();
        state.reload_config();
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .any(|message| message.typ == MessageType::ERROR
                    && message.message.contains("overrides[0].incldues"))
        );
        assert_eq!(
            state.take_config_messages(),
            [] as [lsp_types::ShowMessageParams; 0]
        );
    }
}
#[cfg(test)]
mod dependency_tests {
    use super::*;

    #[test]
    fn package_dependency_requires_matching_owner_name() {
        let by_path = HashMap::from([(PathBuf::from("/workspace/lib/foo.dart"), 0)]);
        let packages = [PackageIdentity {
            name: "workspace".to_string(),
            lib_root: PathBuf::from("/workspace/lib"),
        }];

        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "package:other/foo.dart",
                &by_path,
                &packages,
            ),
            None,
        );
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "package:workspace/foo.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "foo.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );
    }

    #[test]
    fn package_dependency_resolves_matching_workspace_package() {
        let by_path = HashMap::from([(PathBuf::from("/workspace/b/lib/api.dart"), 0)]);
        let packages = [
            PackageIdentity {
                name: "a".to_string(),
                lib_root: PathBuf::from("/workspace/a/lib"),
            },
            PackageIdentity {
                name: "b".to_string(),
                lib_root: PathBuf::from("/workspace/b/lib"),
            },
        ];

        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/a/lib/main.dart"),
                "package:b/api.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );

        let ambiguous = [
            packages[1].clone(),
            PackageIdentity {
                name: "b".to_string(),
                lib_root: PathBuf::from("/workspace/other-b/lib"),
            },
        ];
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/a/lib/main.dart"),
                "package:b/api.dart",
                &by_path,
                &ambiguous,
            ),
            None,
        );
    }

    #[test]
    fn unreadable_nested_pubspec_remains_an_ownership_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path();
        let nested = outer.join("nested");
        std::fs::create_dir_all(nested.join("lib")).unwrap();
        std::fs::write(outer.join("pubspec.yaml"), "name: outer\n").unwrap();
        std::fs::write(nested.join("pubspec.yaml"), [0xff, 0xfe]).unwrap();
        let path = nested.join("lib/main.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse("");
        let files = [ProjectFile {
            path: path.clone(),
            parsed: Arc::new(ParsedFile {
                source: "".into(),
                tree,
            }),
            has_parse_errors: !errors.is_empty(),
        }];

        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let nested_lib = canonical_or_lexical(&nested.join("lib"));
        let owner = packages
            .iter()
            .find(|package| package.lib_root == nested_lib)
            .expect("unreadable nested manifest must retain its package identity");
        assert_eq!(owner.name, "");
    }

    const VERSIONED_SWITCH: &str = "void f(Object o) { switch (o) { case 1 + 2: break; } }\n";

    fn write_package_version(root: &Path, version: &str) {
        let directory = root.join(".dart_tool");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("package_config.json");
        let modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        std::fs::write(
            &path,
            format!(
                r#"{{"configVersion":2,"packages":[{{"name":"fixture","rootUri":"../","packageUri":"lib/","languageVersion":"{version}"}}]}}"#
            ),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(modified + std::time::Duration::from_secs(1)),
            )
            .unwrap();
    }

    fn switch_has_pattern(program: Program<'_>) -> bool {
        use falcon_syntax::cst::{FunctionBody, Stmt, SwitchCaseKind, TopLevelDecl};
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function")
        };
        let Some(FunctionBody::Block(block)) = function.body() else {
            panic!("expected function body")
        };
        let Stmt::Switch(switch) = block.stmts().next().unwrap() else {
            panic!("expected switch")
        };
        matches!(
            switch.cases().next().unwrap().cases().next().unwrap(),
            SwitchCaseKind::Pattern(..)
        )
    }

    #[test]
    fn package_version_reload_reparses_only_changed_effective_versions() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let config_path = dir.path().join("falcon.json");
            let config = if semantic {
                r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#
            } else {
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#
            };
            std::fs::write(&config_path, config).unwrap();
            write_package_version(dir.path(), "2.19");
            let unrelated = dir.path().join("nested");
            std::fs::create_dir_all(unrelated.join("lib")).unwrap();
            write_package_version(&unrelated, "2.19");
            let uri = format!("file://{}", dir.path().join("main.dart").display());
            let marker_uri = format!("file://{}", dir.path().join("marker.dart").display());
            let unrelated_uri = format!("file://{}", unrelated.join("lib/other.dart").display());
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            let initial = state.open(&uri, VERSIONED_SWITCH.to_string(), Some(1));
            state.open(
                &marker_uri,
                format!("// @dart = 2.19\n{VERSIONED_SWITCH}"),
                Some(1),
            );
            state.open(&unrelated_uri, VERSIONED_SWITCH.to_string(), Some(1));
            assert!(!initial.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].parse_count, 1);

            write_package_version(dir.path(), "3.0");
            let published = state.reload_config();
            let diagnostics = &published.iter().find(|(key, _)| key == &uri).unwrap().1;
            assert!(diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            assert_eq!(state.documents[&unrelated_uri].parse_count, 1);
            assert!(!switch_has_pattern(
                state.documents[&marker_uri].parsed.program()
            ));
            state.analyze(&uri);
            state.reload_config();
            assert_eq!(state.documents[&uri].parse_count, 2);
            write_package_version(dir.path(), "3.1");
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 3);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            if semantic {
                let snapshot = state.semantic_snapshot.as_ref().unwrap();
                let file = &snapshot.files[snapshot.by_path[&state.documents[&uri].path]];
                assert!(switch_has_pattern(file.program()));
                assert!(file.has_parse_errors);
            }

            write_package_version(dir.path(), "2.19");
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            assert!(state.change(&uri, VERSIONED_SWITCH.to_string(), Some(2)));
            let diagnostics = state.analyze(&uri);
            assert!(!diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].parse_count, 5);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            assert_eq!(state.documents[&unrelated_uri].parse_count, 1);
        }
    }

    #[test]
    fn disk_collection_discovers_nested_versions_before_parsing_and_indexing() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        let mut paths = Vec::with_capacity(2);
        for (package, version) in [("legacy", "2.19"), ("modern", "3.0")] {
            let root = dir.path().join(package);
            std::fs::create_dir_all(root.join("lib")).unwrap();
            write_package_version(&root, version);
            let path = root.join("lib/case.dart");
            std::fs::write(&path, VERSIONED_SWITCH).unwrap();
            paths.push(canonical_or_lexical(&path));
        }
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        state.open(&uri, "void main() {}\n".to_string(), Some(1));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        for (path, pattern) in paths.iter().zip([false, true]) {
            let file = &snapshot.files[snapshot.by_path[path]];
            assert_eq!(switch_has_pattern(file.program()), pattern);
            assert_eq!(file.has_parse_errors, pattern);
        }
        let (files, _) = state.collect_project_files(ProjectView::CrossFile);
        for (path, pattern) in paths.iter().zip([false, true]) {
            let file = files.iter().find(|file| &file.path == path).unwrap();
            assert_eq!(switch_has_pattern(file.program()), pattern);
            assert_eq!(file.has_parse_errors, pattern);
        }
        assert_eq!(state.documents[&uri].parse_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn materialized_path_reparses_at_new_owner_version_once() {
        for event in ["analyze", "save", "change"] {
            let dir = tempfile::tempdir().unwrap();
            let config_path = dir.path().join("falcon.json");
            std::fs::write(
                &config_path,
                r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
            )
            .unwrap();
            write_package_version(dir.path(), "3.0");
            let real = dir.path().join("real");
            std::fs::create_dir_all(real.join("lib")).unwrap();
            write_package_version(&real, "2.19");
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(real.join("lib"), &link).unwrap();
            let path = link.join("new.dart");
            let uri = format!("file://{}", path.display());
            let marker_path = link.join("marker.dart");
            let marker_uri = format!("file://{}", marker_path.display());
            let marker = format!("// @dart = 2.19\n{VERSIONED_SWITCH}");
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            state.open(&uri, VERSIONED_SWITCH.to_string(), Some(1));
            state.open(&marker_uri, marker.clone(), Some(1));
            assert!(switch_has_pattern(state.documents[&uri].parsed.program()));
            assert!(!switch_has_pattern(
                state.documents[&marker_uri].parsed.program()
            ));
            std::fs::write(&path, VERSIONED_SWITCH).unwrap();
            std::fs::write(&marker_path, &marker).unwrap();
            state.watched_files_changed(&[path.clone(), marker_path.clone()]);
            let diagnostics = match event {
                "save" => state.save(&uri, None),
                "change" => {
                    state.change(&uri, VERSIONED_SWITCH.to_string(), Some(2));
                    state.analyze(&uri)
                }
                _ => state.analyze(&uri),
            };
            assert!(!diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].path, canonical_or_lexical(&path));
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 2);
        }
    }

    #[test]
    fn cross_file_collection_includes_pubspec_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        let manifest_path = dir.path().join("pubspec.yaml");
        std::fs::write(&config_path, "{}").unwrap();
        let source = "name: workspace\nvalue: '$Simple ${Braced}'\n";
        std::fs::write(&manifest_path, source).unwrap();
        std::fs::write(dir.path().join("main.dart"), "void main() {}\n").unwrap();
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);

        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        let manifest = files
            .iter()
            .find(|file| file.path == canonical_or_lexical(&manifest_path))
            .expect("pubspec.yaml must be included in the cross-file project view");
        assert_eq!(manifest.source(), source);
        assert!(!manifest.has_parse_errors);
        assert!(manifest.program().declarations().is_empty());
        assert_eq!(
            manifest
                .tree()
                .tokens(manifest.source())
                .next()
                .unwrap()
                .text(),
            "name"
        );
        assert_eq!(
            manifest
                .tree()
                .tokens(manifest.source())
                .last()
                .unwrap()
                .kind(),
            falcon_syntax::TokenKind::Eof
        );
        assert_eq!(
            manifest
                .tree()
                .identifier_tokens(manifest.source())
                .map(falcon_syntax::cst::SyntaxToken::text)
                .collect::<Vec<_>>(),
            ["name", "workspace", "value", "Simple", "Braced"]
        );
    }

    #[test]
    fn opening_files_outside_cross_file_view_preserves_nullable_diagnostics() {
        for opened in ["external", "excluded", "dependency"] {
            let dir = tempfile::tempdir().unwrap();
            let app = dir.path().join("app");
            let flutter = dir.path().join("flutter");
            let sdk = flutter.join("bin/cache/dart-sdk/lib");
            std::fs::create_dir_all(sdk.join("core")).unwrap();
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(app.join("lib")).unwrap();
            let dependency = dir.path().join("dep/lib/api.dart");
            std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
            let dependency_source = "int value() => 1;\n";
            std::fs::write(&dependency, dependency_source).unwrap();
            let config = write_nullable_dependency_config(&app, &sdk, &flutter);
            let path = app.join("lib/main.dart");
            let source = "import 'package:dep/api.dart';\nvoid _take(int? value) {}\nvoid main() { _take(value()); }\n";
            std::fs::write(&path, source).unwrap();
            let other = match opened {
                "excluded" => app.join("lib/excluded.dart"),
                "dependency" => dependency,
                _ => dir.path().join("external.dart"),
            };
            let other_source = if opened == "dependency" {
                dependency_source
            } else {
                "void unrelated() {}\n"
            };
            std::fs::write(&other, other_source).unwrap();
            let mut state = LspState::new_with_sdk_locator(Some(config), |config| {
                falcon_analyze::DartSdk::load(
                    &config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"),
                )
            });
            if opened == "excluded" {
                state
                    .config
                    .files
                    .includes
                    .extend_excludes(&[other.to_string_lossy().into_owned()]);
            }
            let uri = format!("file://{}", path.display());
            state.open(&uri, source.into(), Some(1));
            assert!(state.environment.has_sdk());
            state.cross_file_pass();
            assert!(
                state.documents[&uri]
                    .last_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
            );

            let other_uri = format!("file://{}", other.display());
            state.open(&other_uri, other_source.into(), Some(1));
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert!(snapshot.by_path.contains_key(&canonical_or_lexical(&other)));
            let builds = state.semantic_snapshot_build_count;
            // An unrelated external buffer can reuse the loaded SDK layer.
            // Removed package sources conservatively require a fresh layer.
            if opened == "external" {
                std::fs::remove_file(sdk.join("core/core.dart")).unwrap();
            }
            let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
            assert!(complete);
            state.ensure_cross_file_indexes(&files);
            assert!(
                state
                    .cross_file_index_cache
                    .as_ref()
                    .unwrap()
                    .indexes
                    .identities
                    .has_sdk_sources()
            );
            let index_builds = state.cross_file_index_build_count;
            state.cross_file_pass();
            assert!(
                state.documents[&uri]
                    .last_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
            );
            assert_eq!(state.semantic_snapshot_build_count, builds);
            state.cross_file_pass();
            assert_eq!(state.cross_file_index_build_count, index_builds);
            assert_eq!(state.semantic_snapshot_build_count, builds);
            if opened != "external" {
                assert_nullable_dependency_refresh(&mut state, &uri, source, &dir);
            }
        }
    }

    fn write_nullable_dependency_config(app: &Path, sdk: &Path, flutter: &Path) -> PathBuf {
        std::fs::write(
            sdk.join("libraries.json"),
            r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            sdk.join("core/core.dart"),
            "library dart.core; class Object {} abstract final class int {}",
        )
        .unwrap();
        std::fs::write(
                app.join(".dart_tool/package_config.json"),
                format!(
                    r#"{{"configVersion":2,"flutterRoot":"file://{}","packages":[
                        {{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"}},
                        {{"name":"dep","rootUri":"../../dep","packageUri":"lib/","languageVersion":"3.9"}}
                    ]}}"#,
                    flutter.display(),
                ),
            )
            .unwrap();
        let config = app.join("falcon.json");
        std::fs::write(
            &config,
            r#"{
                    "linter":{"enabled":false},
                    "cross-file":{"rules":{"recommended":false,
                        "correctness":{"unnecessary-nullable":"warn"}}}
                }"#,
        )
        .unwrap();
        config
    }

    fn assert_nullable_dependency_refresh(
        state: &mut LspState,
        uri: &str,
        source: &str,
        dir: &tempfile::TempDir,
    ) {
        let external_loads = state.cross_file_external_load_count;
        assert!(external_loads > 0);
        for version in [2, 3] {
            assert!(state.change(
                uri,
                format!("{source}// body edit {version}\n"),
                Some(version),
            ));
            state.analyze(uri);
            state.cross_file_pass();
            assert_eq!(state.cross_file_external_load_count, external_loads);
            assert!(
                state.documents[uri]
                    .last_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
            );
        }
        let added = dir.path().join("dep/lib/added.dart");
        std::fs::write(&added, "int addedValue() => 1;\n").unwrap();
        let changed_imports = source
            .replace("package:dep/api.dart", "package:dep/added.dart")
            .replace("_take(value())", "_take(addedValue())");
        assert!(state.change(uri, changed_imports, Some(4)));
        state.analyze(uri);
        state.cross_file_pass();
        assert_eq!(state.cross_file_external_load_count, external_loads + 1);
        assert!(
            state
                .cross_file_external_cache
                .as_ref()
                .unwrap()
                .libraries
                .libraries()
                .iter()
                .any(|library| {
                    library
                        .files
                        .iter()
                        .any(|file| file.path == canonical_or_lexical(&added))
                })
        );
        assert!(
            state.documents[uri]
                .last_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
        );
        std::fs::write(&added, "int? addedValue() => null;\n").unwrap();
        state.watched_files_changed(std::slice::from_ref(&added));
        state.cross_file_pass();
        assert_eq!(state.cross_file_external_load_count, external_loads + 2);
        assert!(
            state.documents[uri]
                .last_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule != "unnecessary-nullable")
        );
    }

    #[test]
    fn cross_file_external_cache_uses_watched_path_snapshot() {
        let path = PathBuf::from("/missing/dependency.dart");
        let cache = CrossFileExternalCache {
            roots: Vec::new(),
            libraries: ExternalLibraries::default(),
            indexes: None,
            watched_paths: HashSet::from([path.clone()]),
        };
        assert!(cache.loaded(&path));
        assert!(!cache.loaded(Path::new("/missing/other.dart")));
    }

    #[test]
    fn cross_file_external_cache_invalidates_watched_excluded_companions() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join("lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("dep")).unwrap();
        let config = app.join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,
                "correctness":{"unnecessary-nullable":"warn"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            app.join("lib/main.dart"),
            "library; part 'excluded.dart'; void main() {}",
        )
        .unwrap();
        let companion = app.join("lib/excluded.dart");
        // Enhanced parts can introduce dependencies without changing their owner.
        std::fs::write(
            &companion,
            "part of 'main.dart'; import '../../dep/first.dart';",
        )
        .unwrap();
        let first = dir.path().join("dep/first.dart");
        let second = dir.path().join("dep/second.dart");
        std::fs::write(&first, "class First {}").unwrap();
        std::fs::write(&second, "class Second {}").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        state
            .config
            .files
            .includes
            .extend_excludes(&[companion.to_string_lossy().into_owned()]);
        let has_external = |state: &LspState, path: &Path| {
            let path = canonical_or_lexical(path);
            state
                .cross_file_external_cache
                .as_ref()
                .unwrap()
                .libraries
                .libraries()
                .iter()
                .any(|library| library.files.iter().any(|file| file.path == path))
        };
        state.cross_file_diagnostics();
        assert!(has_external(&state, &first));
        assert!(!has_external(&state, &second));
        let loads = state.cross_file_external_load_count;
        std::fs::write(
            &companion,
            "part of 'main.dart'; import '../../dep/second.dart';",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&companion));
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_external_load_count, loads + 1);
        assert!(has_external(&state, &second));
        assert!(!has_external(&state, &first));
    }

    #[test]
    fn cross_file_indexes_cache_tracks_disk_revisions_and_file_set() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,
                "correctness":{"unnecessary-nullable":"warn"}}}}"#,
        )
        .unwrap();
        let path = dir.path().join("main.dart");
        std::fs::write(&path, "void _take(int? value) {} void main() { _take(1); }").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 1);
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 1);
        std::fs::write(
            &path,
            "void _take(int? value) {} void main() { _take(null); }",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&path));
        let diagnostics = state.cross_file_diagnostics();
        assert!(
            diagnostics
                .values()
                .flatten()
                .all(|diagnostic| diagnostic.rule != "unnecessary-nullable")
        );
        assert_eq!(state.cross_file_index_build_count, 2);
        let peer = dir.path().join("peer.dart");
        std::fs::write(&peer, "void peer() {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&peer));
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 3);
        std::fs::remove_file(&peer).unwrap();
        state.watched_files_changed(std::slice::from_ref(&peer));
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 4);
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 4);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workspace_publishes_canonical_cross_file_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("lib")).unwrap();
        std::fs::write(
            workspace.join("falcon.json"),
            r#"{
            "linter": { "enabled": false },
            "cross-file": { "rules": {
                "recommended": false,
                "correctness": { "unused-files": "warn" }
            } }
        }"#,
        )
        .unwrap();
        std::fs::write(workspace.join("lib/orphan.dart"), "class Orphan {}\n").unwrap();
        std::fs::write(workspace.join("lib/main.dart"), "void main() {}\n").unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&workspace, &alias).unwrap();
        let uri = format!("file://{}", alias.join("lib/orphan.dart").display());
        let mut state = LspState::new(Some(alias.join("falcon.json")));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state.open(&uri, "class Orphan {}\n".to_string(), Some(1));

        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("cross-file diagnostics must publish through the original symlink URI");
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule == "unused-files")
            .expect("canonical project path must find the orphan diagnostic");
        assert_eq!(
            diagnostic.file_path,
            canonical_or_lexical(&workspace.join("lib/orphan.dart")).to_string_lossy()
        );
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);

        std::fs::write(
            workspace.join("lib/main.dart"),
            "import 'orphan.dart';\nvoid main() { Orphan(); }\n",
        )
        .unwrap();
        state.watched_files_changed(&[alias.join("lib/main.dart")]);
        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("the same symlink URI must receive cleared diagnostics");
        assert!(!diagnostics.iter().any(|diag| diag.rule == "unused-files"));
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn untouched_materialized_buffer_keeps_one_canonical_project_entry() {
        for (resolve, parent_segment) in
            [(true, false), (false, false), (true, true), (false, true)]
        {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("workspace");
            std::fs::create_dir_all(workspace.join("lib/deep")).unwrap();
            let config_path = workspace.join("falcon.json");
            let config = serde_json::json!({
                "linter": {"rules": {"recommended": false,
                    "suspicious": {"avoid-ignoring-return-values": if resolve { "warn" } else { "off" }}}},
                "cross-file": {"rules": {"recommended": false,
                    "correctness": {"unused-files": "warn"}}}
            });
            std::fs::write(&config_path, config.to_string()).unwrap();
            let main = workspace.join("lib/main.dart");
            std::fs::write(&main, "void main() {}\n").unwrap();
            let alias = dir.path().join("alias");
            let target = if parent_segment {
                workspace.join("lib/deep")
            } else {
                workspace.clone()
            };
            std::os::unix::fs::symlink(target, &alias).unwrap();
            let relative = if parent_segment {
                "../orphan.dart"
            } else {
                "lib/orphan.dart"
            };
            let uri = format!("file://{}", alias.join(relative).display());
            let main_uri = format!("file://{}", main.display());
            let buffer = "int api() => 1;\n";
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            assert_eq!(state.resolve_semantics, resolve);
            state.open(&main_uri, "void main() {}\n".into(), Some(1));
            state.open(&uri, buffer.into(), Some(1));
            assert!(state.document(&uri).unwrap().unresolved_path);
            let path = workspace.join("lib/orphan.dart");
            std::fs::write(&path, "int diskOnly() => 2;\n").unwrap();
            state.watched_files_changed(std::slice::from_ref(&path));
            state.change(&main_uri, "void main() { print(1); }\n".into(), Some(2));
            state.analyze(&main_uri);
            let canonical = canonical_or_lexical(&path);
            let doc = state.document(&uri).unwrap();
            assert!(!doc.unresolved_path);
            assert_eq!(doc.path, canonical);
            assert_eq!(doc.parse_count, 1);
            for view in [ProjectView::Semantic, ProjectView::CrossFile] {
                let (files, _) = state.collect_project_files(view);
                assert_eq!(files.len(), 2);
                let file = files.iter().find(|file| file.path == canonical).unwrap();
                assert_eq!(file.source(), buffer);
            }
            let published = state.cross_file_pass();
            let (_, diagnostics) = published.iter().find(|(key, _)| key == &uri).unwrap();
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == "unused-files"
                    && diagnostic.file_path == canonical.to_string_lossy()
            }));
        }
    }

    #[cfg(unix)]
    #[test]
    fn saving_materialized_symlink_document_refreshes_project_identity() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("lib")).unwrap();
        let config_path = workspace.join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{
                "linter": { "rules": {
                    "recommended": false,
                    "suspicious": { "avoid-ignoring-return-values": "warn" }
                } },
                "cross-file": { "rules": {
                    "recommended": false,
                    "correctness": { "unused-files": "warn" }
                } }
            }"#,
        )
        .unwrap();
        std::fs::write(workspace.join("lib/main.dart"), "void main() {}\n").unwrap();
        let alias = workspace.join("alias");
        std::os::unix::fs::symlink(workspace.join("lib"), &alias).unwrap();
        let path = workspace.join("lib/orphan.dart");
        let uri = format!("file://{}", alias.join("orphan.dart").display());
        let source = "int api() => 1;\n";
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state.open(&uri, source.to_string(), Some(1));
        let old_path = state.document(&uri).unwrap().path.clone();
        std::fs::write(&path, source).unwrap();
        state.save(&uri, None);

        let canonical = canonical_or_lexical(&path);
        assert_ne!(old_path, canonical);
        assert_eq!(state.document(&uri).unwrap().path, canonical);
        let (files, _) = state.collect_project_files(ProjectView::Semantic);
        assert_eq!(
            files.len(),
            2,
            "saved alias must not duplicate the disk file"
        );
        assert!(files.iter().all(|file| file.path != old_path));
        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("saved alias must receive cross-file diagnostics");
        assert!(diagnostics.iter().any(|diag| {
            diag.rule == "unused-files" && diag.file_path == canonical.to_string_lossy()
        }));
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);

        let main_uri = format!("file://{}", workspace.join("lib/main.dart").display());
        state.open(
            &main_uri,
            "import 'orphan.dart';\nvoid main() { api(); }\n".to_string(),
            Some(1),
        );
        state.change(&uri, "void api() {}\n".to_string(), Some(2));
        assert!(
            state.affected_open_uris(&uri).contains(&main_uri),
            "dependency graph must use the saved canonical identity"
        );
    }

    #[test]
    fn opening_document_flushes_debounced_dependents_before_cross_file_publish() {
        use lsp_server::{Notification, Request};
        use lsp_types::NumberOrString;
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        let config_path = write_debounced_publish_config(dir.path());
        let a_uri = format!("file://{}", dir.path().join("a.dart").display());
        let b_uri = format!("file://{}", dir.path().join("b.dart").display());
        let c_uri = format!("file://{}", dir.path().join("c.dart").display());
        let a_source = "void api() {}\nvoid unusedA() {}\n";
        let b_source = "import 'a.dart';\nvoid main() { api(); }\n";
        std::fs::write(dir.path().join("a.dart"), a_source).unwrap();
        std::fs::write(dir.path().join("b.dart"), b_source).unwrap();
        let (client, worker) = spawn_debounced_publish_server(config_path);
        let receive_through = |id: i32| receive_published_through(&client, id);
        let notify = |method: &str, params| {
            client
                .sender
                .send(Notification::new(method.to_string(), params).into())
                .unwrap();
        };
        let barrier = |id: i32| {
            client
                .sender
                .send(Request::new(id.into(), "falcon/barrier".to_string(), ()).into())
                .unwrap();
            receive_through(id)
        };
        client
            .sender
            .send(
                Request::new(
                    0.into(),
                    "initialize".to_string(),
                    json!({"capabilities": {}}),
                )
                .into(),
            )
            .unwrap();
        receive_through(0);
        notify("initialized", json!({}));
        for (uri, text) in [(&a_uri, a_source), (&b_uri, b_source)] {
            notify(
                "textDocument/didOpen",
                json!({"textDocument": {
                    "uri": uri, "languageId": "dart", "version": 1, "text": text
                }}),
            );
        }
        let code = Some(NumberOrString::String(
            "avoid-ignoring-return-values".to_string(),
        ));
        let initial = barrier(1);
        let initial_b = initial
            .iter()
            .rev()
            .find(|params| params.uri.as_str() == b_uri)
            .expect("B must publish its initial diagnostics");
        assert!(
            initial_b
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != code)
        );
        notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": a_uri, "version": 2},
                "contentChanges": [{"text": "int api() => 1;\nvoid unusedA() {}\n"}]
            }),
        );
        notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": c_uri, "languageId": "dart", "version": 1, "text": "void c() {}\n"
            }}),
        );
        let published = barrier(2);
        let refreshed = published
            .iter()
            .rev()
            .find(|params| params.uri.as_str() == b_uri)
            .expect("opening C must publish B's pending semantic refresh");
        assert!(
            refreshed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code)
        );
        client
            .sender
            .send(Request::new(3.into(), "shutdown".to_string(), ()).into())
            .unwrap();
        receive_through(3);
        notify("exit", json!({}));
        worker.join().unwrap();
    }

    fn spawn_debounced_publish_server(
        config_path: PathBuf,
    ) -> (lsp_server::Connection, std::thread::JoinHandle<()>) {
        use lsp_server::Connection;
        use std::time::Duration;

        let (server, client) = Connection::memory();
        let worker = std::thread::spawn(move || {
            let options = crate::server::ServerOptions {
                debounce: Duration::from_secs(60),
                config_path: Some(config_path),
            };
            crate::server::run_with_connection(&server, &options).unwrap();
        });
        (client, worker)
    }

    fn write_debounced_publish_config(root: &Path) -> PathBuf {
        let config_path = root.join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{
                "linter": { "rules": {
                    "recommended": false,
                    "suspicious": { "avoid-ignoring-return-values": "warn" }
                } },
                "cross-file": { "rules": {
                    "recommended": false,
                    "correctness": { "unused-code": "warn" }
                } }
            }"#,
        )
        .unwrap();
        config_path
    }

    fn receive_published_through(
        client: &lsp_server::Connection,
        id: i32,
    ) -> Vec<lsp_types::PublishDiagnosticsParams> {
        use lsp_server::Message;
        use lsp_types::PublishDiagnosticsParams;
        use std::time::Duration;

        let mut published = Vec::new();
        loop {
            match client
                .receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
            {
                Message::Response(response) => {
                    assert_eq!(response.id, id.into());
                    break;
                }
                Message::Notification(notification) => {
                    assert_eq!(notification.method, "textDocument/publishDiagnostics");
                    published.push(
                        serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                            .unwrap(),
                    );
                }
                other @ Message::Request(_) => panic!("unexpected server message: {other:?}"),
            }
        }
        published
    }

    #[test]
    fn file_uri_decodes_percent_encoded_path() {
        assert_eq!(
            uri_to_path("file:///workspace/with%20space/main.dart"),
            PathBuf::from("/workspace/with space/main.dart")
        );
    }
}
#[cfg(test)]
mod environment_regressions {
    use super::*;

    fn state(root: &Path) -> LspState {
        let config = root.join("falcon.json");
        std::fs::write(&config,
            r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#).unwrap();
        let mut state = LspState::new(Some(config));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state
    }

    #[test]
    fn environment_failures_use_show_message_warning_queue_once() {
        for name in [
            ".dart_tool/package_config.json",
            "pubspec.lock",
            "../dep/lib/api.dart",
        ] {
            let (directory, mut state, uri, _) = dependency_fixture(false);
            assert_eq!(
                state.take_config_messages(),
                [] as [lsp_types::ShowMessageParams; 0]
            );
            let path = directory.path().join("app").join(name);
            std::fs::write(&path, "[").unwrap();
            state.watched_files_changed(&[path]);
            state.change(
                &uri,
                "import 'package:dep/api.dart'; void main() {}".into(),
                Some(2),
            );
            state.ensure_semantic_snapshot();
            let messages = state.take_config_messages();
            assert_eq!(messages.len(), 1, "{name}: {messages:?}");
            assert_eq!(messages[0].typ, MessageType::WARNING);
            assert!(
                messages[0]
                    .message
                    .contains(Path::new(name).file_name().unwrap().to_str().unwrap())
            );
            assert_eq!(
                state.take_config_messages(),
                [] as [lsp_types::ShowMessageParams; 0]
            );
        }
    }

    #[test]
    fn watched_file_normalization_preserves_filesystem_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("api.dart");
        std::fs::write(&file, "class C {}").unwrap();
        let expected = std::fs::canonicalize(&file).unwrap();
        let normalized = watched_path(&file);
        assert_eq!(normalized.as_os_str(), expected.as_os_str());
        assert!(normalized.is_file());
        std::fs::remove_file(&file).unwrap();
        assert_eq!(watched_path(&file).as_os_str(), expected.as_os_str());
    }

    #[cfg(unix)]
    #[test]
    fn watched_lazily_discovered_package_lib_alias_retargeting_resets_environment() {
        for canonical_parent in [false, true] {
            let (dir, mut state, _, _) = dependency_fixture(false);
            let home = dir.path().join("home");
            std::os::unix::fs::symlink(dir.path(), &home).unwrap();
            let package = home.join("lazy");
            let first = dir.path().join("first");
            let second = dir.path().join("second");
            for directory in [&package, &first, &second] {
                std::fs::create_dir(directory).unwrap();
            }
            std::fs::write(package.join("pubspec.yaml"), "name: lazy\n").unwrap();
            let lib = package.join("lib");
            std::os::unix::fs::symlink(&first, &lib).unwrap();
            let source = package.join("source.dart");
            assert_eq!(
                state
                    .environment
                    .file_package(&source)
                    .identity()
                    .unwrap()
                    .lib_root,
                std::fs::canonicalize(&first).unwrap()
            );
            let event = if canonical_parent {
                std::fs::canonicalize(&package).unwrap().join("lib")
            } else {
                lib.clone()
            };
            assert!(
                !state
                    .environment_stamps
                    .as_ref()
                    .unwrap()
                    .affected_by(&event)
            );
            let resets = state.external_reset_count;
            std::fs::remove_file(&lib).unwrap();
            std::os::unix::fs::symlink(&second, &lib).unwrap();
            state.watched_files_changed(&[event]);
            assert_eq!(state.external_reset_count, resets + 1);
            assert_eq!(
                state
                    .environment
                    .file_package(&source)
                    .identity()
                    .unwrap()
                    .lib_root,
                std::fs::canonicalize(&second).unwrap()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn watched_package_alias_retargeting_and_deletion_reset_external_facts() {
        for canonical_parent in [false, true] {
            let (dir, mut state, uri, _) = dependency_fixture(false);
            let alias = dir.path().join("alias");
            std::os::unix::fs::symlink(dir.path().join("dep"), &alias).unwrap();
            let config = dir.path().join("app/.dart_tool/package_config.json");
            std::fs::write(&config,
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../alias","packageUri":"lib/"}]}"#).unwrap();
            state.watched_files_changed(&[config]);
            assert!(has_external_class(&state, "Original"));
            let replacement = dir.path().join("replacement");
            std::fs::create_dir_all(replacement.join("lib")).unwrap();
            std::fs::write(replacement.join("lib/api.dart"), "class Replacement {}").unwrap();
            std::fs::remove_file(&alias).unwrap();
            std::os::unix::fs::symlink(&replacement, &alias).unwrap();
            let event = if canonical_parent {
                std::fs::canonicalize(dir.path()).unwrap().join("alias")
            } else {
                alias.clone()
            };
            let resets = state.external_reset_count;
            assert!(
                state
                    .watched_files_changed(&[dir.path().join("unrelated")])
                    .is_empty()
            );
            assert_eq!(state.external_reset_count, resets);
            assert!(has_external_class(&state, "Original"));
            let published = state.watched_files_changed(std::slice::from_ref(&event));
            assert!(
                published
                    .iter()
                    .any(|(published_uri, _)| published_uri == &uri)
            );
            assert_eq!(state.external_reset_count, resets + 1);
            assert!(has_external_class(&state, "Replacement"));
            assert!(!has_external_class(&state, "Original"));
            std::fs::remove_file(&alias).unwrap();
            state.watched_files_changed(&[event]);
            assert_eq!(state.external_reset_count, resets + 2);
            assert!(!has_external_class(&state, "Replacement"));
        }
    }

    #[test]
    fn configuration_events_invalidate_after_discovery_preserves_known_fingerprints() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let app = dir.path().join("app");
        let config = app.join(".dart_tool/package_config.json");
        let mut expected_stamps = state.environment_stamps.clone().unwrap();
        std::fs::create_dir_all(dir.path().join("new/lib")).unwrap();
        std::fs::write(dir.path().join("new/lib/api.dart"), "class Replacement {}").unwrap();
        std::fs::write(&config,
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../new","packageUri":"lib/"}]}"#).unwrap();
        std::fs::create_dir_all(app.join("new_directory")).unwrap();
        let new_uri = format!("file://{}", app.join("new_directory/file.dart").display());
        state.open(&new_uri, "void other() {}".into(), Some(1));
        state.analyze(&new_uri);
        let current_stamps = state.environment.config_stamps();
        assert!(expected_stamps.extend_discovery(current_stamps.clone()));
        assert_eq!(state.environment_stamps.as_ref(), Some(&expected_stamps));
        assert_ne!(state.environment_stamps.as_ref(), Some(&current_stamps));
        assert!(has_external_class(&state, "Original"));
        let resets = state.external_reset_count;
        state.watched_files_changed(&[config]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "Replacement"));
        assert!(!has_external_class(&state, "Original"));
        state.analyze(&uri);
        std::fs::rename(app.join(".dart_tool"), app.join("removed-dart-tool")).unwrap();
        state.watched_files_changed(&[app.join(".dart_tool")]);
        assert!(!has_external_class(&state, "Replacement"));
    }

    #[test]
    fn nested_project_sources_retain_cache_but_excluded_dependencies_invalidate() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let app = dir.path().join("app");
        let core = app.join("packages/core/lib/api.dart");
        std::fs::create_dir_all(core.parent().unwrap()).unwrap();
        std::fs::write(&core, "void coreApi() {} class NestedBefore {}").unwrap();
        let config = app.join(".dart_tool/package_config.json");
        std::fs::write(&config,
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"},{"name":"core","rootUri":"../packages/core","packageUri":"lib/"}]}"#).unwrap();
        state.watched_files_changed(&[config]);
        state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); }".into(), Some(2));
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        let resets = state.external_reset_count;
        assert!(state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); } // pending".into(), Some(3)));
        assert!(state.semantic_snapshot.is_none());
        std::fs::write(&core, "int coreApi() => 1; class NestedBefore {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&core));
        assert!(published.iter().any(|(published_uri, diagnostics)| {
            published_uri == &uri
                && diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        }));
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        assert!(state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); } // next pending".into(), Some(4)));
        assert!(state.semantic_snapshot.is_none());
        let generated = core.with_file_name("new.g.dart");
        std::fs::write(&generated, "class NewlyGenerated {}").unwrap();
        state.watched_files_changed(&[generated]);
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let core_uri = format!("file://{}", core.display());
        state.open(
            &core_uri,
            "int coreApi() => 2; class Buffered {}".into(),
            Some(1),
        );
        state.analyze(&core_uri);
        assert!(
            state
                .watched_files_changed(std::slice::from_ref(&core))
                .is_empty()
        );
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        state.close(&core_uri);
        state.config.files.includes.extend_excludes(&[format!(
            "{}/**",
            canonical_or_lexical(&app.join("packages/core")).display()
        )]);
        state.invalidate_semantic_snapshot();
        state.analyze(&uri);
        assert!(has_external_class(&state, "NestedBefore"));
        std::fs::write(&core, "void coreApi() {} class NestedAfter {}").unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[core]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "NestedAfter"));
        assert!(!has_external_class(&state, "NestedBefore"));
    }

    fn dependency_fixture(hosted: bool) -> (tempfile::TempDir, LspState, String, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        let dependency = dir.path().join("dep/lib/api.dart");
        std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
        std::fs::write(&dependency, "class Original {}").unwrap();
        std::fs::write(app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
        if hosted {
            std::fs::write(
                app.join("pubspec.lock"),
                "packages:\n  dep:\n    source: hosted\n    version: '1.0.0'\n",
            )
            .unwrap();
        }
        let mut state = state(&app);
        let uri = format!("file://{}", app.join("main.dart").display());
        state.open(
            &uri,
            "import 'package:dep/api.dart'; void main() {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        (dir, state, uri, dependency)
    }

    fn has_external_class(state: &LspState, name: &str) -> bool {
        state.external_libraries.libraries().iter().flat_map(|library| library.files.iter())
            .flat_map(|file| file.program().declarations()).any(|declaration| {
                matches!(declaration, falcon_syntax::cst::TopLevelDecl::Class(class) if class.name().name() == name)
            })
    }

    #[test]
    fn deleted_dependency_directories_invalidate_loaded_external_facts() {
        for local in [false, true] {
            let (dir, mut state, uri, dependency) = dependency_fixture(false);
            state.set_dynamic_file_watching(true);
            let deleted = if local {
                let directory = dir.path().join("local/nested");
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join("api.dart"), "class Original {}").unwrap();
                state.change(
                    &uri,
                    "import '../local/nested/api.dart'; void main() {}".into(),
                    Some(2),
                );
                state.ensure_semantic_snapshot();
                directory
            } else {
                dependency.parent().unwrap().to_path_buf()
            };
            assert!(has_external_class(&state, "Original"));
            let resets = state.external_reset_count;
            std::fs::remove_dir_all(&deleted).unwrap();
            let published = state.watched_files_changed(&[deleted]);
            assert_eq!(published.len(), 1);
            assert_eq!(published[0].0, uri);
            assert_eq!(state.external_reset_count, resets + 1);
            assert!(!has_external_class(&state, "Original"));
        }
    }

    #[test]
    fn watched_workspace_part_refreshes_project_without_resetting_external_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let generated = dir.path().join("app/generated.dart");
        std::fs::write(&generated, "part of 'main.dart'; class Before {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&generated));
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part 'generated.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let resets = state.external_reset_count;
        std::fs::write(&generated, "part of 'main.dart'; class After {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&generated));
        assert_eq!(published.len(), 1);
        assert_eq!(state.external_reset_count, resets);
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(Arc::ptr_eq(&identities, &cache.identities));
        assert!(Arc::ptr_eq(&types, &cache.types));
        assert!(Arc::ptr_eq(&signatures, &cache.signatures));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let generated = canonical_or_lexical(&generated);
        assert!(
            snapshot.files[snapshot.by_path[&generated]]
                .source()
                .contains("class After")
        );
    }

    #[test]
    fn part_uri_typing_updates_watches_without_rebuilding_external_indexes() {
        let (_dir, mut state, uri, _) = dependency_fixture(false);
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let builds = state.external_index_build_count;
        for (version, part) in [(2, "../missing-one.dart"), (3, "../missing-two.dart")] {
            let generation = state.watched_dependency_generation();
            state.change(
                &uri,
                format!("import 'package:dep/api.dart'; part '{part}'; void main() {{}}"),
                Some(version),
            );
            state.ensure_semantic_snapshot();
            let cache = state.external_indexes.as_ref().unwrap();
            assert!(Arc::ptr_eq(&identities, &cache.identities));
            assert!(Arc::ptr_eq(&types, &cache.types));
            assert!(Arc::ptr_eq(&signatures, &cache.signatures));
            assert_eq!(state.external_index_build_count, builds);
            assert!(state.watched_dependency_generation() > generation);
            assert!(
                state
                    .watched_dependency_files()
                    .iter()
                    .any(|path| { path.file_name() == Path::new(part).file_name() })
            );
        }
        assert!(
            state
                .watched_dependency_files()
                .windows(2)
                .all(|paths| paths[0] < paths[1])
        );
        let generation = state.watched_dependency_generation();
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part '../missing-two.dart'; void main() { }".into(),
            Some(4),
        );
        state.ensure_semantic_snapshot();
        assert_eq!(state.watched_dependency_generation(), generation);
        assert_eq!(state.external_index_build_count, builds);
    }

    #[cfg(unix)]
    #[test]
    fn watched_workspace_alias_event_refreshes_unopened_project_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        let alias = dir.path().join("workspace-alias");
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let peer = root.join("peer.dart");
        std::fs::write(&peer, "class Before {}").unwrap();
        let mut state = state(&alias);
        assert_eq!(state.workspace_root, canonical_or_lexical(&root));
        let uri = format!("file://{}", alias.join("main.dart").display());
        state.open(&uri, "void main() {}".into(), Some(1));
        state.ensure_semantic_snapshot();
        let resets = state.external_reset_count;
        std::fs::write(&peer, "class After {}").unwrap();
        let published = state.watched_files_changed(&[alias.join("peer.dart")]);
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, uri);
        assert_eq!(state.external_reset_count, resets);
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let peer = canonical_or_lexical(&peer);
        assert!(
            snapshot.files[snapshot.by_path[&peer]]
                .source()
                .contains("class After")
        );
    }

    #[cfg(unix)]
    #[test]
    fn watched_local_alias_changes_and_deletions_invalidate_loaded_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let local = dir.path().join("local");
        let alias = dir.path().join("alias");
        std::fs::create_dir(&local).unwrap();
        std::os::unix::fs::symlink(&local, &alias).unwrap();
        let file = local.join("api.dart");
        std::fs::write(&file, "class Before {}").unwrap();
        state.change(
            &uri,
            "import '../alias/api.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        assert!(has_external_class(&state, "Before"));
        let resets = state.external_reset_count;
        std::fs::write(&file, "class After {}").unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "After"));
        assert!(!has_external_class(&state, "Before"));
        std::fs::remove_file(&file).unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 2);
        assert!(!has_external_class(&state, "After"));
        std::fs::write(&file, "class Recreated {}").unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 3);
        assert!(has_external_class(&state, "Recreated"));
    }

    #[test]
    fn watched_companion_absent_from_external_topology_invalidates_external_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let companion = dir.path().join("companion.dart");
        std::fs::write(&companion, "part of 'app/main.dart'; class Before {}").unwrap();
        let companion = canonical_or_lexical(&companion);
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part '../companion.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        assert!(state.loaded_project_paths().any(|path| path == companion));
        assert!(
            !state
                .external_topology
                .as_ref()
                .unwrap()
                .paths
                .iter()
                .any(|(path, _, _)| path == &companion)
        );
        assert!(
            state
                .semantic_snapshot
                .as_ref()
                .unwrap()
                .by_path
                .contains_key(&companion)
        );
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let resets = state.external_reset_count;
        std::fs::write(&companion, "part of 'app/main.dart'; class After {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&companion));
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, uri);
        assert_eq!(state.external_reset_count, resets + 1);
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(!Arc::ptr_eq(&identities, &cache.identities));
        assert!(!Arc::ptr_eq(&types, &cache.types));
        assert!(!Arc::ptr_eq(&signatures, &cache.signatures));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let file = &snapshot.files[snapshot.by_path[&companion]];
        assert!(file.source().contains("class After"));
        assert!(!file.source().contains("class Before"));
    }

    #[test]
    fn project_edits_and_save_events_reuse_all_external_arcs_without_sdk() {
        let (_dir, mut state, uri, dependency) = dependency_fixture(false);
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let builds = state.external_index_build_count;
        assert!(!state.environment.has_sdk());
        assert!(state.change(
            &uri,
            "import 'package:dep/api.dart'; void main() { final value = Original(); }".into(),
            Some(2)
        ));
        state.analyze(&uri);
        let project = state.documents[&uri].path.clone();
        assert!(state.watched_files_changed(&[project]).is_empty());
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(Arc::ptr_eq(&identities, &cache.identities));
        assert!(Arc::ptr_eq(&types, &cache.types));
        assert!(Arc::ptr_eq(&signatures, &cache.signatures));
        assert_eq!(state.external_index_build_count, builds);
        let resets = state.external_reset_count;
        assert!(
            state
                .watched_files_changed(&[dependency
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("irrelevant.dart")])
                .is_empty()
        );
        assert_eq!(state.external_reset_count, resets);
        std::fs::write(&dependency, "class Replaced {}").unwrap();
        state.watched_files_changed(&[dependency]);
        assert!(has_external_class(&state, "Replaced"));
        assert!(!has_external_class(&state, "Original"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
    }

    #[test]
    fn closed_project_events_refresh_diagnostics_without_rebuilding_external_indexes() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let closed = dir.path().join("app/api.dart");
        std::fs::write(&closed, "void api() {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&closed));
        state.change(
            &uri,
            "import 'api.dart'; import 'package:dep/api.dart'; void main() { api(); }".into(),
            Some(2),
        );
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        let builds = state.external_index_build_count;
        std::fs::write(&closed, "int api() => 1;").unwrap();
        let published = state.watched_files_changed(&[closed]);
        assert!(published.iter().any(|(published_uri, diagnostics)| {
            published_uri == &uri
                && diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        }));
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        assert_eq!(state.external_index_build_count, builds);
    }

    #[test]
    fn immutable_hosted_sources_wait_for_resolution_input_events() {
        let (dir, mut state, uri, dependency) = dependency_fixture(true);
        assert_eq!(state.watched_dependency_roots(), [] as [PathBuf; 0]);
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        std::fs::write(&dependency, "class Modified {}").unwrap();
        state.change(
            &uri,
            "import 'package:dep/api.dart'; void main() { }".into(),
            Some(2),
        );
        state.analyze(&uri);
        assert!(state.watched_files_changed(&[dependency]).is_empty());
        assert!(has_external_class(&state, "Original"));
        assert!(!has_external_class(&state, "Modified"));
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let lock = dir.path().join("app/pubspec.lock");
        let metadata = std::fs::metadata(&lock).unwrap();
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: hosted\n    version: '2.0.0'\n",
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        state.watched_files_changed(&[lock]);
        assert!(has_external_class(&state, "Modified"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
    }

    #[test]
    fn lock_source_and_git_reference_events_refresh_mutable_watch_roots() {
        let (dir, mut state, _uri, dependency) = dependency_fixture(true);
        let lock = dir.path().join("app/pubspec.lock");
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: git\n    description:\n      resolved-ref: first\n",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&lock));
        let dependency_path = canonical_or_lexical(&dependency);
        assert!(
            state
                .watched_dependency_roots()
                .iter()
                .any(|root| dependency_path.starts_with(root))
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        std::fs::write(&dependency, "class NewReference {}").unwrap();
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: git\n    description:\n      resolved-ref: other\n",
        )
        .unwrap();
        state.watched_files_changed(&[lock]);
        assert!(has_external_class(&state, "NewReference"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let overrides = dir.path().join("app/pubspec_overrides.yaml");
        std::fs::write(
            &overrides,
            "dependency_overrides:\n  dep:\n    path: ../dep\n",
        )
        .unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[overrides]);
        assert_eq!(state.external_reset_count, resets + 1);
    }

    #[test]
    fn mutable_events_reload_fixed_timestamp_sources_missing_files_and_replaced_roots() {
        let (dir, mut state, _uri, dependency) = dependency_fixture(false);
        let metadata = std::fs::metadata(&dependency).unwrap();
        std::fs::write(&dependency, "class Replaced {}").unwrap();
        assert_eq!(
            std::fs::metadata(&dependency).unwrap().len(),
            metadata.len()
        );
        std::fs::File::options()
            .write(true)
            .open(&dependency)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(has_external_class(&state, "Replaced"));
        std::fs::remove_file(&dependency).unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(!has_external_class(&state, "Replaced"));
        std::fs::write(
            &dependency,
            "library dep; part 'generated.dart'; class Restored {}",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(has_external_class(&state, "Restored"));
        let part = dependency.with_file_name("generated.dart");
        std::fs::write(&part, "part of dep; class Generated {}").unwrap();
        state.watched_files_changed(&[part]);
        assert!(has_external_class(&state, "Generated"));
        let root = dir.path().join("dep");
        std::fs::rename(&root, dir.path().join("old-dep")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(&dependency, "class NewRoot {}").unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[root, dependency]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "NewRoot"));
        assert!(!has_external_class(&state, "Generated"));
        assert!(!has_external_class(&state, "Restored"));
    }

    #[test]
    fn doc_import_backedges_retain_topology_without_external_index_cache() {
        let (dir, mut state, uri, dependency) = dependency_fixture(false);
        std::fs::write(
            &dependency,
            "/// @docImport '../../app/main.dart';\nlibrary;\nclass Original {}",
        )
        .unwrap();
        state.watched_files_changed(&[dependency]);
        assert!(state.external_indexes.is_none());
        let paths = state.external_topology.as_ref().unwrap().paths.as_ptr();
        let target = canonical_or_lexical(&dir.path().join("app")).join("main.dart");
        assert!(
            state
                .external_topology
                .as_ref()
                .unwrap()
                .project_dependency_targets
                .contains(&target)
        );
        state.change(
            &uri,
            "import 'package:dep/api.dart'; class Editable {}".into(),
            Some(2),
        );
        state.analyze(&uri);
        assert!(state.external_indexes.is_none());
        assert_eq!(
            paths,
            state.external_topology.as_ref().unwrap().paths.as_ptr()
        );
    }

    #[test]
    fn first_package_configuration_refreshes_open_buffer_grammar() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut state = state(dir.path());
            state.resolve_semantics = semantic;
            let path = dir.path().join("main.dart");
            let uri = format!("file://{}", path.display());
            let source = "void f(Object o) { switch (o) { case 1 + 2: break; } }";
            state.open(&uri, source.into(), Some(1));
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::CURRENT
            );
            std::fs::create_dir_all(dir.path().join(".dart_tool")).unwrap();
            std::fs::write(dir.path().join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"","languageVersion":"2.19"}]}"#).unwrap();
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            state.analyze(&uri);
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::new(2, 19)
            );
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert!(!state.environment.has_sdk());
        }
    }

    #[cfg(unix)]
    #[test]
    fn missing_workspace_alias_buffer_refreshes_grammar_and_preserves_uri() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("workspace");
            let alias = dir.path().join("alias");
            std::fs::create_dir(&root).unwrap();
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            let mut state = state(&alias);
            state.resolve_semantics = semantic;
            let uri = format!("file://{}", alias.join("new_directory/main.dart").display());
            let source = "void f(Object o) { switch (o) { case 1 + 2: break; } }";
            state.open(&uri, source.into(), Some(1));
            let expected = canonical_or_lexical(&root).join("new_directory/main.dart");
            assert_eq!(state.documents[&uri].path, expected);
            assert!(state.documents[&uri].unresolved_path);
            assert!(!expected.exists());
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::CURRENT
            );
            std::fs::create_dir(alias.join(".dart_tool")).unwrap();
            let config = alias.join(".dart_tool/package_config.json");
            std::fs::write(&config,
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"","languageVersion":"2.19"}]}"#).unwrap();
            let published = state.watched_files_changed(&[config]);
            assert_eq!(published.len(), 1);
            assert_eq!(published[0].0, uri);
            assert_eq!(state.open_uris(), std::slice::from_ref(&uri));
            assert_eq!(state.documents[&uri].path, expected);
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::new(2, 19)
            );
            assert_eq!(state.documents[&uri].parse_count, 2);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 2);
        }
    }

    #[test]
    fn doc_import_dependencies_refresh_and_yield_to_open_buffers_without_sdk() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        let dependency = dir.path().join("dep/lib/api.dart");
        std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
        std::fs::write(&dependency, "class Original {}").unwrap();
        std::fs::write(app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
        let mut state = state(&app);
        let uri = format!("file://{}", app.join("main.dart").display());
        state.open(
            &uri,
            "/// @docImport 'package:dep/api.dart';\nlibrary;\nvoid main() {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_libraries.libraries().len(), 1);
        let resets = state.external_reset_count;
        std::fs::write(&dependency, "class ChangedDependency {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_reset_count, resets + 1);
        let file = &state.external_libraries.libraries()[0].files[0];
        let falcon_syntax::cst::TopLevelDecl::Class(class) =
            file.program().declarations().next().unwrap()
        else {
            panic!("expected dependency class");
        };
        assert_eq!(class.name().name(), "ChangedDependency");
        let dependency_uri = format!("file://{}", dependency.display());
        state.open(
            &dependency_uri,
            "class UnsavedDependency {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        assert!(state.external_libraries.libraries().is_empty());
        assert!(!state.environment.has_sdk());
    }

    #[test]
    fn divergent_shared_dependency_survives_snapshot_edits_open_close_and_regroup() {
        use falcon_analyze::{ResolvedType, SemanticModel};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let shared = root.join("shared/lib/bar.dart");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        let shared_source = "import 'package:foo/foo.dart'; Foo makeValue() => Foo();";
        std::fs::write(&shared, shared_source).unwrap();
        for copy in ["first", "second"] {
            let foo = root.join(format!("deps/{copy}/lib/foo.dart"));
            std::fs::create_dir_all(foo.parent().unwrap()).unwrap();
            std::fs::write(foo, "class Foo {}").unwrap();
        }
        let configure = |app: &str, copy: &str| {
            let directory = root.join(format!("{app}/.dart_tool"));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("package_config.json");
            std::fs::write(
                &path,
                format!(
                    r#"{{"configVersion":2,"packages":[
                        {{"name":"foo","rootUri":"../../deps/{copy}","packageUri":"lib/"}},
                        {{"name":"bar","rootUri":"../../shared","packageUri":"lib/"}}
                    ]}}"#
                ),
            )
            .unwrap();
            path
        };
        let source = "import 'package:foo/foo.dart'; import 'package:bar/bar.dart'; void main() {}";
        let apps = ["first", "second", "third"].map(|app| {
            configure(app, if app == "first" { "first" } else { "second" });
            let path = root.join(format!("{app}/main.dart"));
            std::fs::write(&path, source).unwrap();
            canonical_or_lexical(&path)
        });
        let uris = apps
            .each_ref()
            .map(|path| format!("file://{}", path.display()));
        let shared = canonical_or_lexical(&shared);
        let shared_uri = format!("file://{}", shared.display());
        let mut state = state(root);
        state.config.files.includes.extend_excludes(&[
            format!("{}/**", canonical_or_lexical(&root.join("deps")).display()),
            format!(
                "{}/**",
                canonical_or_lexical(&root.join("shared")).display()
            ),
        ]);
        for uri in &uris {
            state.open(uri, source.into(), Some(1));
        }
        state.ensure_semantic_snapshot();

        let assert_aliases = |state: &LspState, copies: [&str; 3]| {
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            let functions = apps.each_ref().map(|app| {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                let foo = model.resolve_name("Foo").unwrap();
                let function = model.resolve_value("makeValue").unwrap();
                assert!(foo.is_package("foo", "Foo"));
                assert!(function.is_package("bar", "makeValue"));
                assert_eq!(
                    indexes.signatures.function(&function).unwrap().return_type,
                    ResolvedType::interface_identity(foo)
                );
                function
            });
            for (app, copy) in apps.iter().zip(copies) {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                let foo = model.resolve_name("Foo").unwrap();
                let expected =
                    canonical_or_lexical(&root.join(format!("deps/{copy}/lib/foo.dart")));
                assert_eq!(
                    indexes.identities.declaration_library(&foo),
                    Some(
                        indexes
                            .identities
                            .library(indexes.identities.file(&expected).unwrap())
                    )
                );
            }
            assert_ne!(functions[0], functions[1]);
            if copies[2] == copies[0] {
                assert_eq!(functions[2], functions[0]);
            } else {
                assert_eq!(functions[2], functions[1]);
            }
        };
        assert_aliases(&state, ["first", "second", "second"]);

        let assert_reused_after_edit = |state: &mut LspState, version| {
            let topology = state.external_topology.as_ref().unwrap().paths.as_ptr();
            let cache = state.external_indexes.as_ref().unwrap();
            let identities = Arc::clone(&cache.identities);
            let types = Arc::clone(&cache.types);
            let signatures = Arc::clone(&cache.signatures);
            let builds = state.external_index_build_count;
            assert!(state.change(&uris[0], format!("{source} // {version}"), Some(version)));
            state.analyze(&uris[0]);
            assert_eq!(
                topology,
                state.external_topology.as_ref().unwrap().paths.as_ptr()
            );
            let cache = state.external_indexes.as_ref().unwrap();
            assert!(Arc::ptr_eq(&identities, &cache.identities));
            assert!(Arc::ptr_eq(&types, &cache.types));
            assert!(Arc::ptr_eq(&signatures, &cache.signatures));
            assert_eq!(state.external_index_build_count, builds);
        };
        assert_reused_after_edit(&mut state, 2);
        assert_aliases(&state, ["first", "second", "second"]);

        let buffered = "class Buffered {} Buffered makeValue() => Buffered();";
        state.open(&shared_uri, buffered.into(), Some(1));
        for version in [1, 2] {
            if version == 2 {
                assert!(state.change(&shared_uri, format!("{buffered} // edited"), Some(version)));
            }
            state.ensure_semantic_snapshot();
            assert!(
                state
                    .external_libraries
                    .libraries()
                    .iter()
                    .all(|library| { library.files.iter().all(|file| file.path != shared) })
            );
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            let owner = SemanticModel::new(
                &shared,
                &snapshot.files[snapshot.by_path[&shared]].parsed,
                &indexes.identities,
                Some(&indexes.types),
            );
            let buffered = owner.resolve_name("Buffered").unwrap();
            let function = owner.resolve_value("makeValue").unwrap();
            for app in &apps {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                assert_eq!(model.resolve_name("Buffered"), Some(buffered.clone()));
                assert_eq!(model.resolve_value("makeValue"), Some(function.clone()));
                assert_eq!(
                    indexes.signatures.function(&function).unwrap().return_type,
                    ResolvedType::interface_identity(buffered.clone())
                );
            }
        }
        state.close(&shared_uri);
        state.ensure_semantic_snapshot();
        assert_aliases(&state, ["first", "second", "second"]);
        assert_reused_after_edit(&mut state, 3);

        let configuration = configure("third", "first");
        state.watched_files_changed(&[configuration]);
        state.ensure_semantic_snapshot();
        assert_aliases(&state, ["first", "second", "first"]);
        assert_reused_after_edit(&mut state, 4);
        assert_aliases(&state, ["first", "second", "first"]);

        for uri in &uris {
            state.close(uri);
        }
        configure("third", "second");
        let foo_only = "import 'package:foo/foo.dart';";
        for (app, text) in apps.iter().zip([foo_only, foo_only, source]) {
            std::fs::write(app, text).unwrap();
        }
        std::fs::write(
            shared.with_file_name("other.dart"),
            "import 'package:foo/foo.dart'; Foo otherValue() => Foo();",
        )
        .unwrap();
        state.reset_external_environment();
        state.refresh_environment(apps.iter().map(PathBuf::as_path));
        state.open(&uris[2], source.into(), Some(1));
        let first_source =
            format!("{foo_only} import 'package:bar/bar.dart'; import 'package:bar/other.dart';");
        state.open(&uris[0], first_source, Some(1));
        let second_source = format!("{foo_only} import 'package:bar/other.dart';");
        state.open(&uris[1], second_source.clone(), Some(1));
        state.ensure_semantic_snapshot();
        let primary_function = |state: &LspState| {
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            SemanticModel::new(
                &apps[2],
                &snapshot.files[snapshot.by_path[&apps[2]]].parsed,
                &indexes.identities,
                Some(&indexes.types),
            )
            .resolve_value("makeValue")
            .unwrap()
        };
        let before = primary_function(&state);
        let builds = state.external_index_build_count;
        let library_count = state.external_libraries.libraries().len();
        assert!(state.change(
            &uris[1],
            format!("{second_source} import 'package:bar/bar.dart';"),
            Some(2),
        ));
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_libraries.libraries().len(), library_count);
        assert!(state.external_index_build_count > builds);
        let after = primary_function(&state);
        assert_ne!(before, after);
        // Compare the same importing context: a path-only model chooses the
        // shared file's primary copy, which need not be the third app's copy.
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let project_files: Vec<_> = snapshot.files.iter().collect();
        let fresh_external = ExternalIndexCache::build(ExternalSources {
            libraries: &state.external_libraries,
            environment: &state.environment,
        });
        let (fresh, _, _) = state.build_semantic_indexes(
            &project_files,
            &state.external_libraries,
            Some(&fresh_external),
        );
        for app in &apps {
            let parsed = &snapshot.files[snapshot.by_path[app]].parsed;
            let cached = SemanticModel::new(
                app,
                parsed,
                &snapshot.indexes.identities,
                Some(&snapshot.indexes.types),
            );
            let rebuilt = SemanticModel::new(app, parsed, &fresh.identities, Some(&fresh.types));
            assert_eq!(
                cached.resolve_value("makeValue"),
                rebuilt.resolve_value("makeValue")
            );
        }
    }

    #[test]
    fn opening_dependency_parts_preserves_defining_files_and_buffered_parts() {
        for warm in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let app = dir.path().join("app");
            let dep = dir.path().join("dep/lib");
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(&dep).unwrap();
            std::fs::write(app.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
            std::fs::write(dep.join("bridge.dart"), "export 'leaf.dart';").unwrap();
            let defining = dep.join("leaf.dart");
            let part = dep.join("leaf_part.dart");
            std::fs::write(
                &defining,
                "library leaf; part 'leaf_part.dart'; class Leaf {}",
            )
            .unwrap();
            std::fs::write(&part, "part of leaf; class OnDisk {}").unwrap();
            let mut state = state(&app);
            let app_uri = format!("file://{}", app.join("main.dart").display());
            let part_uri = format!("file://{}", part.display());
            let defining = canonical_or_lexical(&defining);
            let part = canonical_or_lexical(&part);
            let buffered = "part of leaf; class Unsaved {}";
            if !warm {
                state.open(&part_uri, buffered.into(), Some(1));
            }
            state.open(
                &app_uri,
                "import 'package:dep/bridge.dart';".into(),
                Some(1),
            );
            if warm {
                state.ensure_semantic_snapshot();
                state.open(&part_uri, buffered.into(), Some(1));
            }
            state.ensure_semantic_snapshot();
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert!(snapshot.files.iter().any(|file| file.path == defining));
            let parts: Vec<_> = snapshot
                .files
                .iter()
                .filter(|file| file.path == part)
                .collect();
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].source(), buffered);
            assert!(state.external_libraries.libraries().iter().all(|library| {
                library
                    .files
                    .iter()
                    .all(|file| file.path != defining && file.path != part)
            }));
            let defining_parse = Arc::clone(&snapshot.files[snapshot.by_path[&defining]].parsed);
            let parses = state.disk_parse_count;
            let reads = state.disk_read_count;
            assert!(parses > 0, "the unopened defining companion was not parsed");
            assert!(state.change(
                &app_uri,
                "import 'package:dep/bridge.dart'; void main() {}".into(),
                Some(2),
            ));
            state.analyze(&app_uri);
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let companion = &snapshot.files[snapshot.by_path[&defining]];
            assert_eq!(
                companion.source(),
                "library leaf; part 'leaf_part.dart'; class Leaf {}"
            );
            assert!(Arc::ptr_eq(&defining_parse, &companion.parsed));
            assert_eq!(
                state.disk_parse_count, parses,
                "unchanged companion was reparsed"
            );
            assert_eq!(
                state.disk_read_count, reads,
                "edit read or checked metadata for an unchanged companion"
            );
            state.close(&part_uri);
            state.ensure_semantic_snapshot();
            let leaf = state
                .external_libraries
                .libraries()
                .iter()
                .find(|library| library.files[0].path == defining)
                .unwrap();
            assert_eq!(leaf.files.len(), 2);
            assert_eq!(leaf.files[1].path, part);
        }
    }
}
