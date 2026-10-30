use super::*;
use lsp_types::Position;

fn fixture() -> (tempfile::TempDir, LspState, String) {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"style":{"unnecessary-new":"warn"}}}}"#,
    )
    .unwrap();
    let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
    state.configure_code_actions(&serde_json::json!({
        "capabilities": {"workspace": {"applyEdit": true}}
    }));
    let uri = format!("file://{}", directory.path().join("main.dart").display());
    state.open(
        &uri,
        "void main() { final x = new Object(); }".to_owned(),
        Some(1),
    );
    (directory, state, uri)
}

fn params(uri: &str, only: Option<&str>) -> CodeActionParams {
    let mut value = serde_json::json!({
        "textDocument": { "uri": uri },
        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 100 } },
        "context": { "diagnostics": [] }
    });
    if let Some(kind) = only {
        value["context"]["only"] = serde_json::json!([kind]);
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn requesting_actions_preserves_published_diagnostic_cache() {
    let (_directory, mut state, uri) = fixture();
    let published = &mut state.documents.get_mut(&uri).unwrap().last_diagnostics;
    assert_eq!(published.len(), 1);
    published[0].rule = "avoid-unused-files";
    published[0].message = "Published cross-file diagnostic".to_owned();
    let actions = state
        .code_actions(&params(&uri, Some("quickfix")), &|| false)
        .unwrap();
    assert_eq!(actions.len(), 1);
    let cached = &state.documents[&uri].last_diagnostics;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].rule, "avoid-unused-files");
    assert_eq!(cached[0].message, "Published cross-file diagnostic");
}

#[test]
fn unfiltered_fix_all_is_lazy_when_client_can_resolve_edits() {
    let (_directory, mut state, uri) = fixture();
    state.configure_code_actions(&serde_json::json!({
        "capabilities": { "textDocument": { "codeAction": { "dataSupport": true, "resolveSupport": { "properties": ["edit"] } } } }
    }));
    let builds = state.semantic_snapshot_build_count;
    let actions = state.code_actions(&params(&uri, None), &|| false).unwrap();
    let action = actions
        .into_iter()
        .find(|action| {
            action
                .kind
                .as_ref()
                .is_some_and(|kind| kind.as_str() == SAFE_FIX_ALL)
        })
        .unwrap();
    assert!(action.edit.is_none());
    assert!(action.data.is_some());
    assert_eq!(state.semantic_snapshot_build_count, builds);
    let resolved = state.resolve_code_action(action, &|| false).unwrap();
    assert!(resolved.edit.unwrap().changes.is_some());
}

#[test]
fn lazy_actions_require_data_support_and_edit_resolution() {
    let (_directory, mut state, uri) = fixture();
    for data_support in [None, Some(false), Some(true)] {
        let mut capabilities = serde_json::json!({
            "capabilities": { "workspace": {"applyEdit": true}, "textDocument": { "codeAction": { "resolveSupport": { "properties": ["edit"] } } } }
        });
        if let Some(data_support) = data_support {
            capabilities["capabilities"]["textDocument"]["codeAction"]["dataSupport"] =
                serde_json::json!(data_support);
        }
        state.configure_code_actions(&capabilities);
        let actions = state
            .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].edit.is_none());
        assert_eq!(actions[0].command.is_none(), data_support == Some(true));
    }
    state.configure_code_actions(&serde_json::json!({
        "capabilities": { "textDocument": { "codeAction": { "dataSupport": true } } }
    }));
    assert!(!state.code_action_resolve);
}

#[test]
fn fallback_commands_require_negotiated_apply_edit_support() {
    let (_directory, mut state, uri) = fixture();
    for workspace in [
        serde_json::json!({}),
        serde_json::json!({"applyEdit": false}),
    ] {
        state.configure_code_actions(&serde_json::json!({
            "capabilities": {"workspace": workspace}
        }));
        for kind in [SAFE_FIX_ALL, UNSAFE_FIX_ALL] {
            assert_eq!(
                state
                    .code_actions(&params(&uri, Some(kind)), &|| false)
                    .unwrap(),
                [] as [lsp_types::CodeAction; 0]
            );
        }
        // Quick fixes still return ordinary edits without server application.
        assert_eq!(
            state
                .code_actions(&params(&uri, Some("quickfix")), &|| false)
                .unwrap()
                .len(),
            1
        );
    }
    state.configure_code_actions(&serde_json::json!({
        "capabilities": {
            "textDocument": {"codeAction": {"dataSupport": true, "resolveSupport": {"properties": ["edit"]}}}
        }
    }));
    let actions = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert!(actions[0].data.is_some());
    assert!(actions[0].command.is_none());
}

#[test]
fn lazy_safe_fix_all_requires_an_effective_safe_fix() {
    let (_directory, mut state, uri) = fixture();
    state.code_action_resolve = true;
    state.documents.get_mut(&uri).unwrap().last_diagnostics[0]
        .fix
        .as_mut()
        .unwrap()
        .kind = FixKind::Unsafe;
    assert_eq!(
        state
            .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
            .unwrap(),
        [] as [lsp_types::CodeAction; 0]
    );
    let actions = state.code_actions(&params(&uri, None), &|| false).unwrap();
    assert!(actions.iter().any(|action| {
        action
            .kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == UNSAFE_FIX_ALL)
    }));
    assert!(!actions.iter().any(|action| {
        action
            .kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == SAFE_FIX_ALL)
    }));
}

#[test]
fn clients_without_resolve_receive_lazy_commands_and_compatible_quickfixes() {
    let (_directory, mut state, uri) = fixture();
    for document_changes in [false, true] {
        state.configure_code_actions(&serde_json::json!({
            "capabilities": { "workspace": { "applyEdit": true, "workspaceEdit": { "documentChanges": document_changes } } }
        }));
        let actions = state.code_actions(&params(&uri, None), &|| false).unwrap();
        assert!(actions.iter().any(|action| {
            action
                .kind
                .as_ref()
                .is_some_and(|kind| kind.as_str() == SAFE_FIX_ALL)
        }));
        for action in actions {
            if action.kind.as_ref().unwrap() == &CodeActionKind::QUICKFIX {
                let edit = action.edit.unwrap();
                assert_eq!(edit.document_changes.is_some(), document_changes);
                assert_eq!(edit.changes.is_some(), !document_changes);
            } else {
                assert!(action.edit.is_none());
                assert!(action.data.is_none());
                assert_eq!(action.command.unwrap().command, FIX_ALL_COMMAND);
            }
        }
    }
}

#[test]
fn dirty_requests_wait_for_publish_without_reanalyzing() {
    let (_directory, mut state, uri) = fixture();
    state.change(
        &uri,
        "void main() { final y = new Object(); }".to_owned(),
        Some(2),
    );
    let builds = state.semantic_snapshot_build_count;
    let analyses = state.documents[&uri].analyze_count;
    for _ in 0..3 {
        assert_eq!(
            state.code_actions(&params(&uri, None), &|| false).unwrap(),
            [] as [lsp_types::CodeAction; 0]
        );
    }
    assert_eq!(state.semantic_snapshot_build_count, builds);
    assert_eq!(state.documents[&uri].analyze_count, analyses);
    state.analyze(&uri);
    assert_ne!(
        state
            .code_actions(&params(&uri, Some("quickfix")), &|| false)
            .unwrap(),
        [] as [lsp_types::CodeAction; 0]
    );
}

#[test]
fn cursor_at_diagnostic_end_offers_quick_fix() {
    let (_directory, mut state, uri) = fixture();
    let diagnostic = &state.documents[&uri].last_diagnostics[0];
    let end = diagnostic
        .format_lsp_in(&LineIndex::new(state.documents[&uri].text()))
        .range
        .end;
    let mut request = params(&uri, Some("quickfix"));
    request.range = Range::new(end, end);
    assert_eq!(state.code_actions(&request, &|| false).unwrap().len(), 1);
}

#[test]
fn resolving_rejects_changed_cancelled_and_syntax_error_documents() {
    let (_directory, mut state, uri) = fixture();
    state.code_action_resolve = true;
    let action = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap()
        .remove(0);
    assert!(
        state
            .resolve_code_action(action.clone(), &|| true)
            .unwrap_err()
            .contains("cancelled")
    );
    state.change(
        &uri,
        "void main( { final y = new Object(); }".to_owned(),
        Some(2),
    );
    assert!(
        state
            .resolve_code_action(action.clone(), &|| false)
            .unwrap_err()
            .contains("changed")
    );
    state.analyze(&uri);
    let mut current = action;
    current.data.as_mut().unwrap()["version"] = serde_json::json!(2);
    assert!(
        state
            .resolve_code_action(current, &|| false)
            .unwrap_err()
            .contains("syntax errors")
    );
    assert_eq!(
        state
            .code_actions(&params(&uri, Some("quickfix")), &|| false)
            .unwrap(),
        [] as [lsp_types::CodeAction; 0]
    );
    let actions = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert!(actions[0].edit.is_none());
    assert!(
        actions[0]
            .disabled
            .as_ref()
            .unwrap()
            .reason
            .contains("syntax errors")
    );
}

#[test]
fn resolving_rejects_failed_semantic_snapshot() {
    let (_directory, mut state, uri) = fixture();
    state.code_action_resolve = true;
    let action = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap()
        .remove(0);
    state.semantic_snapshot_error = Some(std::io::Error::other("snapshot failed"));
    assert!(
        state
            .resolve_code_action(action, &|| false)
            .unwrap_err()
            .contains("valid semantic snapshot")
    );
}

#[test]
fn fix_all_reuses_published_external_graph_and_indexes() {
    let directory = tempfile::tempdir().unwrap();
    let app = directory.path().join("app");
    std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
    let dependency = directory.path().join("dep/lib/api.dart");
    std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
    std::fs::write(&dependency, "class Example {}").unwrap();
    std::fs::write(app.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
    let config = app.join("falcon.json");
    std::fs::write(&config,
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"},"style":{"unnecessary-new":"warn"}}},"cross-file":{"enabled":false}}"#).unwrap();
    let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
    state.configure_code_actions(&serde_json::json!({
        "capabilities": {"workspace": {"applyEdit": true}}
    }));
    let uri = format!("file://{}", app.join("main.dart").display());
    state.open(&uri, "import 'package:dep/api.dart'; void main() { final a = new Example(); final b = new Example(); }".to_owned(), Some(1));
    let identities = std::sync::Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
    let builds = state.external_index_build_count;
    let parses = state.disk_parse_count;
    let actions = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap();
    assert_eq!(actions.len(), 1);
    let action = state
        .resolve_code_action(
            CodeAction {
                data: actions[0]
                    .command
                    .as_ref()
                    .unwrap()
                    .arguments
                    .as_ref()
                    .unwrap()
                    .first()
                    .cloned(),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
    assert!(action.edit.is_some());
    assert!(std::sync::Arc::ptr_eq(
        &identities,
        &state.external_indexes.as_ref().unwrap().identities
    ));
    assert_eq!(state.external_index_build_count, builds);
    assert_eq!(state.disk_parse_count, parses);
    let unrelated = format!("file://{}", app.join("unrelated.dart").display());
    state.open(&unrelated, "void unrelated() {}".to_owned(), Some(1));
    state.close(&unrelated);
    assert!(state.semantic_snapshot.is_none());
    let snapshots = state.semantic_snapshot_build_count;
    let actions = state
        .code_actions(&params(&uri, Some(SAFE_FIX_ALL)), &|| false)
        .unwrap();
    assert_eq!(actions.len(), 1);
    let action = state
        .resolve_code_action(
            CodeAction {
                data: actions[0]
                    .command
                    .as_ref()
                    .unwrap()
                    .arguments
                    .as_ref()
                    .unwrap()
                    .first()
                    .cloned(),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
    assert!(action.edit.is_some());
    assert!(state.semantic_snapshot.is_some());
    assert_eq!(state.semantic_snapshot_build_count, snapshots + 1);
}

#[test]
fn minimal_edits_preserve_common_text_and_utf8_boundaries() {
    for (source, replacement, start, end, text) in [
        (
            "a😀z",
            "a😁z",
            Position::new(0, 1),
            Position::new(0, 3),
            "😁",
        ),
        (
            "a😀z",
            "a😀!z",
            Position::new(0, 3),
            Position::new(0, 3),
            "!",
        ),
        ("a😀z", "az", Position::new(0, 1), Position::new(0, 3), ""),
        (
            "one\nsame\nlast",
            "one\nnew\nlast",
            Position::new(1, 0),
            Position::new(1, 4),
            "new",
        ),
        (
            "a\r\nb",
            "a\nb",
            Position::new(0, 1),
            Position::new(1, 0),
            "\n",
        ),
        (
            "a\nb",
            "a\r\nb",
            Position::new(0, 1),
            Position::new(1, 0),
            "\r\n",
        ),
    ] {
        let edit = minimal_edit(source, replacement.to_owned(), &LineIndex::new(source));
        assert_eq!(edit.range, Range::new(start, end));
        assert_eq!(edit.new_text, text);
        let mut applied = source.to_owned();
        let start = falcon_diagnostics::lsp_position_to_byte(source, edit.range.start);
        let end = falcon_diagnostics::lsp_position_to_byte(source, edit.range.end);
        applied.replace_range(start..end, &edit.new_text);
        assert_eq!(applied, replacement);
    }
}
#[test]
fn secondary_root_actions_use_its_rules_and_fix_safety() {
    for fix in ["unsafe", "none"] {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        for root in [&first, &second] {
            std::fs::create_dir(root).unwrap();
        }
        std::fs::write(
            first.join("falcon.json"),
            r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        std::fs::write(
                second.join("falcon.json"),
                format!(
                    r#"{{"linter":{{"rules":{{"recommended":false,"style":{{"unnecessary-new":{{"level":"error","fix":"{fix}"}},"prefer-single-quotes":"warn"}}}}}},"cross-file":{{"enabled":false}}}}"#
                ),
            )
            .unwrap();
        let mut state = LspState::new_from(None, |_| None, first.clone());
        state.initialize_workspace_roots(vec![first.clone(), second.clone()]);
        state.configure_code_actions(&serde_json::json!({
            "capabilities": {"workspace": {"applyEdit": true, "workspaceEdit": {"documentChanges": true}}}
        }));
        let source = "void main() {\n  final x = new Object();\n  print(\"hello\");\n}\n";
        for (root, secondary) in [(&first, false), (&second, true)] {
            let uri = format!("file://{}", root.join("main.dart").display());
            let diagnostics = state.open(&uri, source.to_owned(), Some(7));
            let params = serde_json::from_value(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 20, "character": 0 } },
                    "context": { "diagnostics": [], "only": ["quickfix", SAFE_FIX_ALL, UNSAFE_FIX_ALL] }
                })).unwrap();
            let actions = state.code_actions(&params, &|| false).unwrap();
            if !secondary {
                assert!(diagnostics.is_empty(), "{diagnostics:?}");
                assert!(actions.is_empty(), "{actions:?}");
                continue;
            }
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == "unnecessary-new"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error
            }));
            assert_eq!(
                actions.len(),
                if fix == "unsafe" { 4 } else { 3 },
                "{actions:?}"
            );
            for mut action in actions {
                if let Some(command) = action.command.take() {
                    action.data = command.arguments.unwrap().into_iter().next();
                    action = state.resolve_code_action(action, &|| false).unwrap();
                }
                let kind = action.kind.as_ref().unwrap().as_str();
                let removes_new = if kind == CodeActionKind::QUICKFIX.as_str() {
                    let diagnostic = &action.diagnostics.as_ref().unwrap()[0];
                    let removes_new = diagnostic.code
                        == Some(lsp_types::NumberOrString::String("unnecessary-new".into()));
                    assert_eq!(action.is_preferred, Some(!removes_new));
                    assert_eq!(action.title.contains("(unsafe)"), removes_new);
                    removes_new
                } else {
                    assert!(kind == SAFE_FIX_ALL || kind == UNSAFE_FIX_ALL);
                    kind == UNSAFE_FIX_ALL && fix == "unsafe"
                };
                let replaces_quotes = kind != CodeActionKind::QUICKFIX.as_str() || !removes_new;
                let mut expected = source.to_owned();
                if removes_new {
                    expected = expected.replace("new ", "");
                }
                if replaces_quotes {
                    expected = expected.replace("\"hello\"", "'hello'");
                }
                let workspace_edit = action.edit.as_ref().expect("action has an edit");
                let Some(DocumentChanges::Edits(documents)) = &workspace_edit.document_changes
                else {
                    panic!("expected versioned document edits");
                };
                assert_eq!(documents.len(), 1);
                assert_eq!(documents[0].text_document.uri.as_str(), uri);
                assert_eq!(documents[0].text_document.version, Some(7));
                let mut applied = source.to_owned();
                for edit in documents[0].edits.iter().rev() {
                    let OneOf::Left(edit) = edit else {
                        panic!("expected text edit");
                    };
                    let start = falcon_diagnostics::lsp_position_to_byte(source, edit.range.start);
                    let end = falcon_diagnostics::lsp_position_to_byte(source, edit.range.end);
                    applied.replace_range(start..end, &edit.new_text);
                }
                assert_eq!(applied, expected, "{action:?}");
            }
        }
    }
}
