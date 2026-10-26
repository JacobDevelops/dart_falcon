use super::*;

#[test]
fn initialize_workspace_folders_take_precedence_over_root_uri() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    let legacy = directory.path().join("legacy");
    let watching = FileWatching::new(&serde_json::json!({
        "workspaceFolders": [
            {"uri": file_uri(&first).unwrap(), "name": "first"},
            {"uri": file_uri(&second).unwrap(), "name": "second"},
            {"uri": file_uri(&first).unwrap(), "name": "duplicate"},
            {"uri": "untitled:buffer", "name": "virtual"}
        ],
        "rootUri": file_uri(&legacy).unwrap()
    }));
    assert_eq!(
        watching.workspace_roots,
        vec![canonical_or_lexical(&first), canonical_or_lexical(&second)]
    );
    for folders in [serde_json::Value::Null, serde_json::json!([])] {
        let watching = FileWatching::new(&serde_json::json!({
            "workspaceFolders": folders,
            "rootUri": file_uri(&legacy).unwrap()
        }));
        assert_eq!(
            watching.workspace_roots,
            vec![canonical_or_lexical(&legacy)]
        );
    }
}

#[test]
fn initialize_analyzes_all_folders_and_reports_secondary_configuration_errors() {
    use serde_json::json;

    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    let legacy = directory.path().join("legacy");
    std::fs::create_dir_all(first.join("lib/src")).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(
        first.join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
    )
    .unwrap();
    let secondary_config = second.join("falcon.json");
    std::fs::write(&secondary_config, r#"{"linter":{"enabledd":true}}"#).unwrap();
    std::fs::write(
        legacy.join("falcon.json"),
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let source = "class Used {}\n";
    for name in ["used.dart", "unused.dart"] {
        std::fs::write(first.join("lib/src").join(name), source).unwrap();
    }
    // This closed file must enter the analysis graph from the second root.
    // Otherwise used.dart is incorrectly reported as unreachable.
    std::fs::write(
        second.join("main.dart"),
        "import '../first/lib/src/used.dart';\nvoid main() { Used(); }\n",
    )
    .unwrap();
    let (server, client) = Connection::memory();
    let worker = std::thread::spawn(move || {
        let options = ServerOptions {
            config_path: None,
            debounce: Duration::ZERO,
        };
        let state = LspState::new_with_sdk_locator(None, |_| None);
        run_with_state(server, options, state).unwrap();
    });
    let receive = || {
        client
            .receiver
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
    };
    client
        .sender
        .send(
            Request::new(
                1.into(),
                "initialize".to_string(),
                json!({
                    "capabilities": {},
                    "rootUri": file_uri(&legacy).unwrap(),
                    "workspaceFolders": [
                        {"uri": file_uri(&first).unwrap(), "name": "first"},
                        {"uri": file_uri(&second).unwrap(), "name": "second"}
                    ]
                }),
            )
            .into(),
        )
        .unwrap();
    let Message::Response(response) = receive() else {
        panic!("expected initialize response");
    };
    assert!(response.error.is_none());
    client
        .sender
        .send(ServerNotification::new("initialized".into(), json!({})).into())
        .unwrap();
    loop {
        if let Message::Notification(notification) = receive()
            && notification.method == ShowMessage::METHOD
        {
            let message: lsp_types::ShowMessageParams =
                serde_json::from_value(notification.params).unwrap();
            if message.typ == lsp_types::MessageType::ERROR {
                assert!(message.message.contains(secondary_config.to_str().unwrap()));
                break;
            }
        }
    }
    for (name, unused) in [("used.dart", false), ("unused.dart", true)] {
        let uri = file_uri(&first.join("lib/src").join(name)).unwrap();
        client.sender.send(ServerNotification::new(DidOpenTextDocument::METHOD.into(), json!({
            "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": source}
        })).into()).unwrap();
        loop {
            if let Message::Notification(notification) = receive()
                && notification.method == PublishDiagnostics::METHOD
            {
                let params: PublishDiagnosticsParams =
                    serde_json::from_value(notification.params).unwrap();
                if params.uri == uri {
                    assert_eq!(
                        params.diagnostics.iter().any(|diagnostic| {
                            diagnostic.code
                                == Some(lsp_types::NumberOrString::String("unused-files".into()))
                        }),
                        unused,
                        "{name}: {:?}",
                        params.diagnostics
                    );
                    break;
                }
            }
        }
    }
    client
        .sender
        .send(Request::new(2.into(), "shutdown".into(), ()).into())
        .unwrap();
    loop {
        if let Message::Response(response) = receive()
            && response.id == 2.into()
        {
            assert!(response.error.is_none());
            break;
        }
    }
    client
        .sender
        .send(ServerNotification::new("exit".into(), json!(null)).into())
        .unwrap();
    worker.join().unwrap();
}

#[cfg(test)]
mod debounce_tests {
    use super::*;

    #[test]
    fn clearing_last_dirty_document_cancels_deadline() {
        let mut debounce = Debounce::default();
        debounce.schedule("file:///b.dart".into(), Duration::from_secs(1));
        debounce.schedule("file:///a.dart".into(), Duration::from_secs(1));
        let deadline = debounce.deadline;
        debounce.clear("file:///b.dart");
        assert_eq!(debounce.deadline, deadline);
        debounce.clear("file:///a.dart");
        assert!(debounce.deadline.is_none());
        assert!(debounce.dirty.is_empty());
    }

    #[test]
    fn drain_deduplicates_documents_and_resets_deadline() {
        let mut debounce = Debounce::default();
        for uri in ["file:///b.dart", "file:///a.dart", "file:///b.dart"] {
            debounce.schedule(uri.into(), Duration::from_secs(1));
        }
        assert_eq!(debounce.drain(), ["file:///a.dart", "file:///b.dart"]);
        assert!(debounce.deadline.is_none());
        assert!(debounce.dirty.is_empty());
    }
}

#[cfg(test)]
mod file_watching_tests {
    use super::*;
    use serde_json::{Value, json};

    const SOURCE: &str = "import 'package:dep/api.dart';\nvoid f() { C x = make(); x; }\n";
    const CONFIG: &str = r#"{"linter":{"rules":{"recommended":false,"style":{"omit-local-variable-types":"warn"},"suspicious":{"avoid-dynamic":"warn"}}},"cross-file":{"enabled":false}}"#;

    struct Client {
        connection: Connection,
        worker: std::thread::JoinHandle<()>,
        directory: tempfile::TempDir,
        workspace: PathBuf,
    }

    impl Client {
        fn start(dynamic: bool, relative: bool) -> Self {
            Self::start_with_initialize(dynamic, relative, json!({}))
        }

        fn start_with_initialize(dynamic: bool, relative: bool, extra: Value) -> Self {
            Self::start_with_configuration(dynamic, relative, extra, CONFIG)
        }

        fn start_with_configuration(
            dynamic: bool,
            relative: bool,
            extra: Value,
            config: &str,
        ) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let workspace = directory.path().join("workspace");
            std::fs::create_dir_all(workspace.join(".dart_tool")).unwrap();
            std::fs::write(workspace.join("falcon.json"), config).unwrap();
            std::fs::write(workspace.join("main.dart"), SOURCE).unwrap();
            for name in ["dep [one]", "dep two"] {
                let library = directory.path().join(name).join("lib");
                std::fs::create_dir_all(&library).unwrap();
                std::fs::write(library.join("api.dart"), "class C {} C make() => C();").unwrap();
            }
            Self::package_config(&workspace, "../../dep%20%5Bone%5D");
            let (server, connection) = Connection::memory();
            let options = ServerOptions {
                debounce: Duration::ZERO,
                config_path: Some(workspace.join("falcon.json")),
            };
            let worker = std::thread::spawn(move || {
                let state = LspState::new_with_sdk_locator(options.config_path.clone(), |_| None);
                run_with_state(server, options, state).unwrap();
            });
            let client = Self {
                connection,
                worker,
                directory,
                workspace,
            };
            let mut initialize = json!({
                "capabilities": {"workspace": {"didChangeWatchedFiles": {
                    "dynamicRegistration": dynamic, "relativePatternSupport": relative
                }}},
                "workspaceFolders": [{"uri": file_uri(&client.workspace).unwrap(), "name": "workspace"}]
            });
            initialize
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            client
                .connection
                .sender
                .send(Request::new(1.into(), "initialize".to_string(), initialize).into())
                .unwrap();
            let Message::Response(response) = client.receive() else {
                panic!("expected initialize response");
            };
            assert!(response.error.is_none());
            client.notify("initialized", json!({}));
            client
        }

        fn package_config(workspace: &Path, root: &str) {
            std::fs::write(
                workspace.join(".dart_tool/package_config.json"),
                json!({
                    "configVersion": 2,
                    "packages": [
                        {"name": "app", "rootUri": "../", "packageUri": ""},
                        {"name": "dep", "rootUri": root, "packageUri": "lib/"}
                    ]
                })
                .to_string(),
            )
            .unwrap();
        }

        fn receive(&self) -> Message {
            self.connection
                .receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
        }

        fn notify(&self, method: &str, params: Value) {
            self.connection
                .sender
                .send(ServerNotification::new(method.to_string(), params).into())
                .unwrap();
        }

        /// A request following the event is an ordering barrier, avoiding sleeps
        /// when asserting that no registration or duplicate publish was sent.
        fn drain(&self) -> Vec<Message> {
            self.connection
                .sender
                .send(Request::new(99.into(), "falcon/testBarrier".to_string(), ()).into())
                .unwrap();
            let mut messages = Vec::new();
            loop {
                let message = self.receive();
                if let Message::Response(response) = &message
                    && response.id == 99.into()
                {
                    break;
                }
                if let Message::Request(request) = &message {
                    self.connection
                        .sender
                        .send(Response::new_ok(request.id.clone(), ()).into())
                        .unwrap();
                }
                messages.push(message);
            }
            messages
        }

        fn open(&self) {
            self.notify(
                "textDocument/didOpen",
                json!({"textDocument": {
                    "uri": file_uri(&self.workspace.join("main.dart")).unwrap(),
                    "languageId": "dart", "version": 1, "text": SOURCE
                }}),
            );
        }

        fn finish(self) {
            self.connection
                .sender
                .send(Request::new(100.into(), "shutdown".to_string(), ()).into())
                .unwrap();
            let Message::Response(response) = self.receive() else {
                panic!("expected shutdown response");
            };
            assert_eq!(response.id, 100.into());
            self.notify("exit", Value::Null);
            self.worker.join().unwrap();
        }
    }

    #[test]
    fn first_open_delivers_environment_failures_to_client_once() {
        for input in [
            ".dart_tool/package_config.json",
            "pubspec.lock",
            "../dep [one]/lib/api.dart",
        ] {
            let client = Client::start(false, false);
            let path = client.workspace.join(input);
            let source = if input.ends_with(".dart") {
                "class Broken {"
            } else {
                "["
            };
            std::fs::write(&path, source).unwrap();
            client.open();
            let opened = client.drain();
            let warnings: Vec<_> = opened
                .iter()
                .filter_map(|message| match message {
                    Message::Notification(notification)
                        if notification.method == ShowMessage::METHOD =>
                    {
                        Some(
                            serde_json::from_value::<lsp_types::ShowMessageParams>(
                                notification.params.clone(),
                            )
                            .unwrap(),
                        )
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(warnings.len(), 1, "{input}: {opened:?}");
            assert_eq!(warnings[0].typ, lsp_types::MessageType::WARNING);
            assert!(
                warnings[0]
                    .message
                    .contains(Path::new(input).file_name().unwrap().to_str().unwrap())
            );
            client.notify(
                "workspace/didChangeWatchedFiles",
                json!({"changes":[{
                    "uri":file_uri(&path).unwrap(), "type":2
                }]}),
            );
            assert!(client.drain().iter().all(|message| !matches!(message,
                Message::Notification(notification) if notification.method == ShowMessage::METHOD
            )));
            client.finish();
        }
    }

    fn registrations(
        messages: &[Message],
    ) -> Vec<lsp_types::DidChangeWatchedFilesRegistrationOptions> {
        messages
            .iter()
            .filter_map(|message| {
                let Message::Request(request) = message else {
                    return None;
                };
                if request.method == UnregisterCapability::METHOD {
                    let params: UnregistrationParams =
                        serde_json::from_value(request.params.clone()).unwrap();
                    assert_eq!(params.unregisterations.len(), 1);
                    assert_eq!(
                        params.unregisterations[0].method,
                        DidChangeWatchedFiles::METHOD
                    );
                    return None;
                }
                assert_eq!(request.method, RegisterCapability::METHOD);
                let params: RegistrationParams =
                    serde_json::from_value(request.params.clone()).unwrap();
                assert_eq!(params.registrations.len(), 1);
                assert_eq!(
                    params.registrations[0].method,
                    DidChangeWatchedFiles::METHOD
                );
                Some(
                    serde_json::from_value(
                        params.registrations[0].register_options.clone().unwrap(),
                    )
                    .unwrap(),
                )
            })
            .collect()
    }

    fn publications(messages: &[Message]) -> Vec<PublishDiagnosticsParams> {
        messages
            .iter()
            .filter_map(|message| {
                let Message::Notification(notification) = message else {
                    return None;
                };
                assert_eq!(notification.method, PublishDiagnostics::METHOD);
                Some(serde_json::from_value(notification.params.clone()).unwrap())
            })
            .collect()
    }

    fn has_rule(publication: &PublishDiagnosticsParams, rule: &str) -> bool {
        publication.diagnostics.iter().any(|diagnostic| {
            matches!(&diagnostic.code, Some(lsp_types::NumberOrString::String(code)) if code == rule)
        })
    }

    #[test]
    fn manifest_change_refreshes_publications_when_rules_config_is_invalid() {
        let client = Client::start_with_configuration(
            true,
            false,
            json!({}),
            r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"depend-on-referenced-packages":"warn"}}}}"#,
        );
        client.drain();
        let config = client.workspace.join("falcon.json");
        let manifest = client.workspace.join("pubspec.yaml");
        std::fs::write(&manifest, "name: app\n").unwrap();
        let uri = file_uri(&client.workspace.join("main.dart")).unwrap();
        client.open();
        let opened = client.drain();
        assert!(
            opened.iter().any(|message| {
                let Message::Notification(notification) = message else {
                    return false;
                };
                if notification.method != PublishDiagnostics::METHOD {
                    return false;
                }
                let publication: PublishDiagnosticsParams =
                    serde_json::from_value(notification.params.clone()).unwrap();
                publication.uri == uri && has_rule(&publication, "depend-on-referenced-packages")
            }),
            "{opened:?}"
        );
        std::fs::write(&config, "{").unwrap();
        std::fs::write(&manifest, "name: app\ndependencies:\n  dep: any\n").unwrap();
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes":[{
                "uri": file_uri(&manifest).unwrap(), "type": 2
            }]}),
        );
        let changed = client.drain();
        assert!(changed.iter().any(|message| matches!(message,
            Message::Notification(notification)
                if notification.method == ShowMessage::METHOD
                    && notification.params["type"] == 1
                    && notification.params["message"].as_str().unwrap().contains("failed to load config")
        )), "{changed:?}");
        assert!(
            changed.iter().any(|message| {
                let Message::Notification(notification) = message else {
                    return false;
                };
                if notification.method != PublishDiagnostics::METHOD {
                    return false;
                }
                let publication: PublishDiagnosticsParams =
                    serde_json::from_value(notification.params.clone()).unwrap();
                publication.uri == uri && !has_rule(&publication, "depend-on-referenced-packages")
            }),
            "{changed:?}"
        );
        client.finish();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workspace_watcher_roots_use_filesystem_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        let alias = dir.path().join("alias");
        let dependency = root.join("dep");
        std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(dependency.join("lib")).unwrap();
        std::fs::write(root.join("falcon.json"), CONFIG).unwrap();
        std::fs::write(root.join("main.dart"), SOURCE).unwrap();
        std::fs::write(
            dependency.join("lib/api.dart"),
            "class C {} C make() => C();",
        )
        .unwrap();
        Client::package_config(&root, "../dep");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(root.join("falcon.json")), |_| None);
        state.open(
            file_uri(&alias.join("main.dart")).unwrap().as_str(),
            SOURCE.to_string(),
            Some(1),
        );
        assert!(
            state
                .watched_dependency_roots()
                .contains(&canonical_or_lexical(&dependency))
        );
        let mut watching = FileWatching::new(&json!({
            "capabilities": {"workspace": {"didChangeWatchedFiles": {
                "dynamicRegistration": true, "relativePatternSupport": true
            }}},
            "workspaceFolders": [{"uri": file_uri(&alias).unwrap(), "name": "workspace"}]
        }));
        let (server, client) = Connection::memory();
        watching.refresh(&server, &state).unwrap();
        let messages: Vec<_> = client.receiver.try_iter().collect();
        let registered = registrations(&messages);
        assert_eq!(registered.len(), 1);
        assert!(
            registered[0]
                .watchers
                .iter()
                .all(|watcher| match &watcher.glob_pattern {
                    GlobPattern::String(_) => true,
                    GlobPattern::Relative(relative) => relative.pattern == "pubspec.yaml",
                })
        );
    }

    #[test]
    fn watcher_registration_respects_each_client_capability_and_does_not_repeat() {
        for (dynamic, relative) in [(false, false), (false, true), (true, false), (true, true)] {
            let client = Client::start(dynamic, relative);
            let initial = client.drain();
            let initial = registrations(&initial);
            assert_eq!(initial.len(), usize::from(dynamic));
            if dynamic {
                assert!(
                    initial[0]
                        .watchers
                        .iter()
                        .all(|watcher| watcher.kind.is_none())
                );
                for pattern in [
                    "**/falcon.json",
                    "**/.falcon.json",
                    "**/.dart_tool/package_config.json",
                    "**/pubspec.{yaml,lock}",
                    "**/pubspec_overrides.yaml",
                    "**/melos.yaml",
                    "**/*.dart",
                ] {
                    assert!(
                        initial[0]
                            .watchers
                            .iter()
                            .any(|watcher| watcher.glob_pattern
                                == GlobPattern::String(pattern.to_string()))
                    );
                }
                if relative {
                    let parent = canonical_or_lexical(client.directory.path());
                    assert!(!parent.join("pubspec.yaml").exists());
                    assert!(initial[0].watchers.iter().any(|watcher| {
                        watcher.glob_pattern
                            == GlobPattern::Relative(RelativePattern {
                                base_uri: OneOf::Right(file_uri(&parent).unwrap()),
                                pattern: "pubspec.yaml".to_string(),
                            })
                    }));
                }
            }
            client.open();
            let opened = client.drain();
            let registrations = registrations(&opened);
            assert_eq!(registrations.len(), usize::from(dynamic && relative));
            if dynamic && relative {
                let root = canonical_or_lexical(&client.directory.path().join("dep [one]"));
                assert!(registrations[0].watchers.iter().any(|watcher| {
                    watcher.glob_pattern
                        == GlobPattern::Relative(RelativePattern {
                            base_uri: OneOf::Right(file_uri(&root).unwrap()),
                            pattern: "**/*".to_string(),
                        })
                }));
                assert!(registrations[0].watchers.iter().any(|watcher| {
                    watcher.glob_pattern
                        == GlobPattern::Relative(RelativePattern {
                            base_uri: OneOf::Right(
                                file_uri(&canonical_or_lexical(client.directory.path())).unwrap(),
                            ),
                            pattern: "*".to_string(),
                        })
                }));
                let methods: Vec<_> = opened
                    .iter()
                    .filter_map(|message| match message {
                        Message::Request(request) => Some(request.method.as_str()),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    methods,
                    [UnregisterCapability::METHOD, RegisterCapability::METHOD]
                );
            }
            assert_eq!(publications(&opened).len(), 1);
            assert!(
                client.drain().is_empty(),
                "unchanged roots must not reregister"
            );
            client.finish();
        }
    }

    #[test]
    fn missing_local_import_gets_exact_watch_and_refreshes_when_created() {
        let client = Client::start(true, true);
        client.drain();
        let local = client.directory.path().join("local");
        std::fs::create_dir(&local).unwrap();
        let file = local.join("api[one].dart");
        let source = "import '../local/api[one].dart'; void f() { C x = make(); x; }";
        std::fs::write(client.workspace.join("main.dart"), source).unwrap();
        client.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": file_uri(&client.workspace.join("main.dart")).unwrap(),
                "languageId": "dart", "version": 1, "text": source
            }}),
        );
        let opened = client.drain();
        assert!(!has_rule(
            &publications(&opened)[0],
            "omit-local-variable-types"
        ));
        let registered = registrations(&opened);
        assert_eq!(registered.len(), 1);
        assert!(registered[0].watchers.iter().any(|watcher| {
            watcher.glob_pattern
                == GlobPattern::Relative(RelativePattern {
                    base_uri: OneOf::Right(file_uri(&canonical_or_lexical(&local)).unwrap()),
                    pattern: "api[[]one[]].dart".to_string(),
                })
        }));
        let pattern = registered[0]
            .watchers
            .iter()
            .find_map(|watcher| match &watcher.glob_pattern {
                GlobPattern::Relative(relative)
                    if relative.base_uri
                        == OneOf::Right(file_uri(&canonical_or_lexical(&local)).unwrap()) =>
                {
                    Some(relative.pattern.clone())
                }
                _ => None,
            })
            .unwrap();
        let matcher = falcon_config::PathFilter::from(vec![pattern]);
        assert!(matcher.admits("api[one].dart"));
        assert!(!matcher.admits("apio.dart"));
        assert!(!matcher.admits("api[other].dart"));
        std::fs::write(&file, "class C {} C make() => C();").unwrap();
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{"uri": file_uri(&file).unwrap(), "type": 1}]}),
        );
        let refreshed = client.drain();
        assert_eq!(publications(&refreshed).len(), 1);
        assert!(has_rule(
            &publications(&refreshed)[0],
            "omit-local-variable-types"
        ));
        assert!(registrations(&refreshed).is_empty());
        client.finish();
    }

    #[test]
    fn loaded_local_libraries_and_companions_are_watched_and_invalidated() {
        for (library_source, source, changed_file) in [
            (
                "class C {} C make() => C();",
                "import '../local [files]/api.dart'; void f() { C x = make(); x; }",
                "api.dart",
            ),
            (
                "part 'impl.dart';",
                "import '../local [files]/api.dart'; void f() { C x = make(); x; }",
                "impl.dart",
            ),
            (
                "",
                "part '../local [files]/impl.dart'; void f() { C x = make(); x; }",
                "impl.dart",
            ),
        ] {
            let client = Client::start(true, true);
            client.drain();
            let local = client.directory.path().join("local [files]");
            std::fs::create_dir_all(&local).unwrap();
            std::fs::write(local.join("api.dart"), library_source).unwrap();
            let part_of = if library_source.is_empty() {
                "../workspace/main.dart"
            } else {
                "api.dart"
            };
            std::fs::write(
                local.join("impl.dart"),
                format!("part of '{part_of}'; class C {{}} C make() => C();"),
            )
            .unwrap();
            std::fs::write(client.workspace.join("main.dart"), source).unwrap();
            client.notify(
                "textDocument/didOpen",
                json!({"textDocument": {
                    "uri": file_uri(&client.workspace.join("main.dart")).unwrap(),
                    "languageId": "dart", "version": 1, "text": source
                }}),
            );
            let opened = client.drain();
            assert!(has_rule(
                &publications(&opened)[0],
                "omit-local-variable-types"
            ));
            let registered = registrations(&opened);
            assert_eq!(registered.len(), 1);
            assert!(registered[0].watchers.iter().any(|watcher| {
                watcher.glob_pattern
                    == GlobPattern::Relative(RelativePattern {
                        base_uri: OneOf::Right(file_uri(&canonical_or_lexical(&local)).unwrap()),
                        pattern: changed_file.to_string(),
                    })
            }));
            assert!(!registered[0].watchers.iter().any(|watcher| {
                matches!(&watcher.glob_pattern, GlobPattern::Relative(relative)
                    if relative.pattern == "**/*"
                        && [local.as_path(), client.directory.path()]
                            .iter()
                            .any(|base| relative.base_uri == OneOf::Right(file_uri(&canonical_or_lexical(base)).unwrap())))
            }));
            let changed = local.join(changed_file);
            let declarations = "class C {} class D {} D make() => D();";
            let replacement = if changed_file == "impl.dart" {
                format!("part of '{part_of}'; {declarations}")
            } else {
                declarations.to_string()
            };
            std::fs::write(&changed, replacement).unwrap();
            client.notify(
                "workspace/didChangeWatchedFiles",
                json!({"changes": [{"uri": file_uri(&changed).unwrap(), "type": 2}]}),
            );
            let refreshed = client.drain();
            assert!(registrations(&refreshed).is_empty());
            let published = publications(&refreshed);
            assert_eq!(published.len(), 1);
            assert!(!has_rule(&published[0], "omit-local-variable-types"));
            client.finish();
        }
    }

    #[test]
    fn watched_event_batch_refreshes_dependency_diagnostics_and_registration_roots() {
        let client = Client::start(true, true);
        client.drain();
        client.open();
        let opened = client.drain();
        assert!(has_rule(
            &publications(&opened)[0],
            "omit-local-variable-types"
        ));
        let dependency = client.directory.path().join("dep [one]/lib/api.dart");
        std::fs::write(&dependency, "class C {} class D {} D make() => D();").unwrap();
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [
                {"uri": file_uri(&dependency).unwrap(), "type": 2},
                {"uri": file_uri(&dependency).unwrap(), "type": 2}
            ]}),
        );
        let changed = client.drain();
        assert!(registrations(&changed).is_empty());
        let published = publications(&changed);
        assert_eq!(
            published.len(),
            1,
            "one event array triggers one diagnostic refresh"
        );
        assert!(!has_rule(&published[0], "omit-local-variable-types"));
        Client::package_config(&client.workspace, "../../dep%20two");
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{
                "uri": file_uri(&client.workspace.join(".dart_tool/package_config.json")).unwrap(),
                "type": 2
            }]}),
        );
        let retargeted = client.drain();
        assert!(has_rule(
            &publications(&retargeted)[0],
            "omit-local-variable-types"
        ));
        let registrations = registrations(&retargeted);
        assert_eq!(registrations.len(), 1);
        let bases: Vec<_> = registrations[0]
            .watchers
            .iter()
            .filter_map(|watcher| match &watcher.glob_pattern {
                GlobPattern::Relative(relative) if relative.pattern == "**/*" => {
                    Some(&relative.base_uri)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            bases,
            [&OneOf::Right(
                file_uri(&canonical_or_lexical(
                    &client.directory.path().join("dep two")
                ))
                .unwrap()
            )]
        );
        client.finish();
    }

    #[test]
    fn configuration_documents_reload_only_on_unwatched_save() {
        for dynamic in [false, true] {
            let client = Client::start(dynamic, false);
            client.drain();
            client.open();
            client.drain();
            let config = client.workspace.join("falcon.json");
            std::fs::write(
                &config,
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
            )
            .unwrap();
            for method in [
                "textDocument/didOpen",
                "textDocument/didChange",
                "textDocument/didClose",
            ] {
                client.notify(method, json!({
                    "textDocument": {"uri": file_uri(&config).unwrap(), "languageId": "json", "version": 2, "text": "{}"},
                    "contentChanges": [{"text": "{}"}]
                }));
                assert!(
                    client.drain().is_empty(),
                    "configuration document {method} must not reload disk state"
                );
            }
            client.notify(
                "textDocument/didSave",
                json!({"textDocument": {"uri": file_uri(&config).unwrap()}}),
            );
            let mut changed = client.drain();
            if dynamic {
                assert!(
                    changed.is_empty(),
                    "watch-capable client saves must not duplicate watcher refreshes"
                );
                client.notify(
                    "workspace/didChangeWatchedFiles",
                    json!({"changes": [{
                        "uri": file_uri(&config).unwrap(), "type": 2
                    }]}),
                );
                changed = client.drain();
            }
            assert!(registrations(&changed).is_empty());
            let published = publications(&changed);
            assert_eq!(published.len(), 1);
            assert!(published[0].diagnostics.is_empty());
            client.finish();
        }
    }

    #[test]
    fn package_config_document_save_retargets_dependencies_without_watcher_support() {
        let client = Client::start(false, false);
        assert!(client.drain().is_empty());
        client.open();
        assert!(has_rule(
            &publications(&client.drain())[0],
            "omit-local-variable-types"
        ));
        let dependency = client.directory.path().join("dep two/lib/api.dart");
        std::fs::write(&dependency, "class C {} class D {} D make() => D();").unwrap();
        Client::package_config(&client.workspace, "../../dep%20two");
        client.notify(
            "textDocument/didSave",
            json!({"textDocument": {
                "uri": file_uri(&client.workspace.join(".dart_tool/package_config.json")).unwrap()
            }}),
        );
        let changed = client.drain();
        assert!(registrations(&changed).is_empty());
        let published = publications(&changed);
        assert_eq!(published.len(), 1);
        assert!(!has_rule(&published[0], "omit-local-variable-types"));
        client.finish();
    }

    #[test]
    fn malformed_optional_initialize_values_do_not_abort_the_server() {
        let client = Client::start_with_initialize(
            true,
            true,
            json!({
                "processId": "ignored unrelated optional value",
                "workspaceFolders": "malformed optional workspace roots",
                "capabilities": {"workspace": {"didChangeWatchedFiles": {
                    "dynamicRegistration": "not a boolean", "relativePatternSupport": true
                }}}
            }),
        );
        assert!(client.drain().is_empty());
        client.open();
        assert_eq!(publications(&client.drain()).len(), 1);
        client.finish();
    }

    #[test]
    fn rejected_registration_disables_watching_and_enables_save_fallback() {
        let client = Client::start(true, true);
        let Message::Request(request) = client.receive() else {
            panic!("expected watcher registration");
        };
        assert_eq!(request.method, RegisterCapability::METHOD);
        client
            .connection
            .sender
            .send(
                Response::new_err(
                    request.id,
                    ErrorCode::MethodNotFound as i32,
                    "registration unsupported".to_string(),
                )
                .into(),
            )
            .unwrap();
        assert!(client.drain().is_empty());
        client.open();
        let opened = client.drain();
        assert!(
            registrations(&opened).is_empty(),
            "rejected registration must not remain active"
        );
        assert_eq!(publications(&opened).len(), 1);
        let config = client.workspace.join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        client.notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": file_uri(&config).unwrap()}}),
        );
        let saved = client.drain();
        assert!(registrations(&saved).is_empty());
        let published = publications(&saved);
        assert_eq!(published.len(), 1);
        assert!(published[0].diagnostics.is_empty());
        client.finish();
    }

    #[test]
    fn dependency_root_delete_and_recreate_with_glob_metacharacters_refreshes_diagnostics() {
        let client = Client::start(true, true);
        client.drain();
        client.open();
        assert!(has_rule(
            &publications(&client.drain())[0],
            "omit-local-variable-types"
        ));
        let root = client.directory.path().join("dep [one]");
        std::fs::remove_dir_all(&root).unwrap();
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{
                "uri": file_uri(&root).unwrap(), "type": 3
            }]}),
        );
        let deleted = client.drain();
        let published = publications(&deleted);
        assert_eq!(published.len(), 1);
        assert!(!has_rule(&published[0], "omit-local-variable-types"));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("lib/api.dart"), "class C {} C make() => C();").unwrap();
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{
                "uri": file_uri(&root).unwrap(), "type": 1
            }]}),
        );
        let recreated = client.drain();
        let published = publications(&recreated);
        assert_eq!(published.len(), 1);
        assert!(has_rule(&published[0], "omit-local-variable-types"));
        client.finish();
    }

    #[test]
    fn empty_and_already_open_project_watcher_batches_publish_nothing() {
        let client = Client::start(true, true);
        client.drain();
        client.open();
        client.drain();
        client.notify("workspace/didChangeWatchedFiles", json!({"changes": []}));
        assert!(client.drain().is_empty());
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{
                "uri": file_uri(&client.workspace.join("main.dart")).unwrap(), "type": 2
            }]}),
        );
        assert!(
            client.drain().is_empty(),
            "an open document's text events own its refresh"
        );
        client.finish();
    }

    #[test]
    fn ignored_watcher_batch_preserves_pending_cross_file_debounce() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = workspace.join("falcon.json");
        std::fs::write(&config,
            r#"{"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#
        ).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        assert!(state.cross_file_rules_enabled());
        let mut debounce = Debounce::default();
        let uri = file_uri(&workspace.join("pending.dart"))
            .unwrap()
            .as_str()
            .to_string();
        debounce.schedule(uri.clone(), Duration::from_secs(30));
        let deadline = debounce.deadline;
        let (server, client) = Connection::memory();
        watched_files_and_publish(
            &server,
            &mut state,
            &mut debounce,
            &[directory.path().join("unrelated.dart")],
        )
        .unwrap();
        assert_eq!(debounce.deadline, deadline);
        assert!(debounce.dirty.contains(&uri));
        assert!(client.receiver.try_recv().is_err());
    }
}

#[test]
fn publish_maps_all_diagnostic_ranges_to_utf16() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
    let uri = file_uri(&directory.path().join("main.dart")).unwrap();
    let source = "// 😀x\n// é😀y\n";
    state.open(uri.as_str(), source.to_owned(), Some(7));
    let diagnostics: Vec<_> = ['x', 'y']
        .into_iter()
        .map(|character| {
            let start = source.find(character).unwrap();
            Diagnostic::new(
                "test-rule",
                falcon_diagnostics::Severity::Warning,
                "message",
                "main.dart",
                falcon_diagnostics::Span {
                    start,
                    end: start + 1,
                },
            )
        })
        .collect();
    let (server, client) = Connection::memory();
    publish(&server, &state, uri.as_str(), &diagnostics).unwrap();
    let Message::Notification(notification) = client.receiver.try_recv().unwrap() else {
        panic!("expected diagnostic publication");
    };
    assert_eq!(notification.method, PublishDiagnostics::METHOD);
    let params: PublishDiagnosticsParams = serde_json::from_value(notification.params).unwrap();
    assert_eq!(params.version, Some(7));
    assert_eq!(params.uri, uri);
    let ranges: Vec<_> = params
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.range)
        .collect();
    assert_eq!(
        ranges,
        [
            lsp_types::Range::new(
                lsp_types::Position::new(0, 5),
                lsp_types::Position::new(0, 6)
            ),
            lsp_types::Range::new(
                lsp_types::Position::new(1, 6),
                lsp_types::Position::new(1, 7)
            ),
        ]
    );
}
