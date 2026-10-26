//! LSP protocol tests: a mock client drives the real server loop over
//! `Connection::memory()`, so the exact production code path is exercised.
//!
//! JSON-RPC 2.0 / LSP 3.17 message shapes are checked by deserializing
//! every server message into the corresponding
//! `lsp_types` type — malformed shapes fail the test at the serde layer.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileChangeType, FileEvent, Hover,
    HoverParams, InitializeResult, Position, PublishDiagnosticsParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, TextDocumentSyncCapability, Uri, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams,
};
use tempfile::TempDir;

use falcon_lsp::{ServerOptions, run_with_connection};

const RECV_TIMEOUT: Duration = Duration::from_secs(5);
const VIOLATING_SRC: &str = "void f() {\n  dynamic x = 1;\n  print(x);\n}\n";
const CLEAN_SRC: &str = "void f() {\n  final int x = 1;\n  print(x);\n}\n";
const DOC_URI: &str = "file:///test/a.dart";
/// Non-compiling Dart: the parser recovers but reports a syntax error.
const SYNTAX_ERR_SRC: &str = "void f() {\n  final int x = ;\n}\n";
/// Syntax error on a line preceded by two astral-plane emoji: the reported
/// column must count UTF-16 code units (each emoji = 2), not scalar values.
const EMOJI_SYNTAX_SRC: &str = "final s = \"\u{1F600}\u{1F600}\"; final int x = ;\n";

struct TestClient {
    client: Connection,
    server: Option<JoinHandle<()>>,
    next_id: i32,
    _config_dir: Option<TempDir>,
}

impl TestClient {
    /// Start the server loop on a memory connection and complete the
    /// initialize handshake. `config_json = None` uses an empty config
    /// (recommended rules enabled) for hermeticity.
    fn start(debounce: Duration, config_json: Option<&str>) -> Self {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("falcon.json");
        fs::write(&config_path, config_json.unwrap_or("{}")).unwrap();

        let (server_conn, client_conn) = Connection::memory();
        let options = ServerOptions {
            debounce,
            config_path: Some(config_path),
        };
        let server = std::thread::spawn(move || {
            run_with_connection(server_conn, options).expect("server loop failed");
        });
        let mut this = Self {
            client: client_conn,
            server: Some(server),
            next_id: 0,
            _config_dir: Some(dir),
        };
        this.initialize();
        this
    }

    fn config_path(&self) -> PathBuf {
        self._config_dir
            .as_ref()
            .unwrap()
            .path()
            .join("falcon.json")
    }

    fn request<P: serde::Serialize>(&mut self, method: &str, params: P) -> RequestId {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.client
            .sender
            .send(Request::new(id.clone(), method.to_string(), params).into())
            .unwrap();
        id
    }

    fn notify<P: serde::Serialize>(&self, method: &str, params: P) {
        self.client
            .sender
            .send(Notification::new(method.to_string(), params).into())
            .unwrap();
    }

    fn recv(&self) -> Message {
        self.client
            .receiver
            .recv_timeout(RECV_TIMEOUT)
            .expect("timed out waiting for server message")
    }

    /// Receive the next message, asserting it is the response to `id`.
    fn recv_response(&self, id: &RequestId) -> lsp_server::Response {
        match self.recv() {
            Message::Response(resp) => {
                assert_eq!(&resp.id, id, "response id must echo the request id");
                resp
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    /// Receive the next `textDocument/publishDiagnostics`, validating its
    /// shape via `lsp_types` deserialization.
    fn recv_publish(&self) -> PublishDiagnosticsParams {
        match self.recv() {
            Message::Notification(note) => {
                assert_eq!(
                    note.method,
                    lsp_types::notification::PublishDiagnostics::METHOD,
                    "expected publishDiagnostics"
                );
                serde_json::from_value(note.params).expect("PublishDiagnosticsParams shape")
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    /// Assert no further message arrives within `window`.
    fn assert_quiet(&self, window: Duration) {
        if let Ok(msg) = self.client.receiver.recv_timeout(window) {
            panic!("expected no message, got {msg:?}");
        }
    }

    fn initialize(&mut self) {
        let id = self.request(
            lsp_types::request::Initialize::METHOD,
            lsp_types::InitializeParams {
                capabilities: lsp_types::ClientCapabilities {
                    workspace: Some(lsp_types::WorkspaceClientCapabilities {
                        apply_edit: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let resp = self.recv_response(&id);
        // Validate the message shape against lsp_types.
        let result: InitializeResult =
            serde_json::from_value(resp.result.expect("initialize must return a result"))
                .expect("InitializeResult shape");
        assert!(
            matches!(
                result.capabilities.text_document_sync,
                Some(TextDocumentSyncCapability::Options(_))
            ),
            "text document sync must be advertised"
        );
        assert!(result.capabilities.hover_provider.is_some());
        assert!(result.capabilities.code_action_provider.is_some());
        assert!(
            result
                .capabilities
                .execute_command_provider
                .as_ref()
                .unwrap()
                .commands
                .iter()
                .any(|command| command == "falcon.fixAll")
        );
        assert_eq!(
            result.capabilities.position_encoding,
            Some(lsp_types::PositionEncodingKind::UTF16),
            "positions must be advertised as UTF-16 so clients decode columns correctly"
        );
        let info = result.server_info.expect("server_info");
        assert_eq!(info.name, "falcon");
        self.notify(
            lsp_types::notification::Initialized::METHOD,
            lsp_types::InitializedParams {},
        );
    }

    fn open(&self, uri: &str, text: &str, version: i32) {
        self.notify(
            lsp_types::notification::DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Uri::from_str(uri).unwrap(),
                    language_id: "dart".to_string(),
                    version,
                    text: text.to_string(),
                },
            },
        );
    }

    fn change(&self, uri: &str, text: &str, version: i32) {
        self.notify(
            lsp_types::notification::DidChangeTextDocument::METHOD,
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Uri::from_str(uri).unwrap(),
                    version,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: text.to_string(),
                }],
            },
        );
    }

    fn shutdown(&mut self) {
        let id = self.request(lsp_types::request::Shutdown::METHOD, ());
        let resp = self.recv_response(&id);
        assert!(resp.error.is_none(), "shutdown must succeed: {resp:?}");
        self.notify(lsp_types::notification::Exit::METHOD, ());
        self.server
            .take()
            .unwrap()
            .join()
            .expect("server thread must exit cleanly after shutdown/exit");
    }
}

fn has_rule(params: &PublishDiagnosticsParams, rule: &str) -> bool {
    params
        .diagnostics
        .iter()
        .any(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(code)) if code == rule))
}

fn code_actions(
    client: &mut TestClient,
    diagnostics: Vec<lsp_types::Diagnostic>,
    only: Option<Vec<&str>>,
    range: lsp_types::Range,
) -> Vec<lsp_types::CodeAction> {
    let id = client.request(
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": DOC_URI },
            "range": range,
            "context": { "diagnostics": diagnostics, "only": only },
        }),
    );
    let response = client.recv_response(&id);
    assert!(response.error.is_none(), "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

fn action_text(action: &lsp_types::CodeAction, version: i32, source: &str) -> String {
    let edit = action.edit.as_ref().expect("action has an edit");
    if let Some(changes) = &edit.changes {
        assert!(edit.document_changes.is_none());
        let edits = &changes[&DOC_URI.parse::<Uri>().unwrap()];
        let mut result = source.to_owned();
        for edit in edits.iter().rev() {
            let start = falcon_diagnostics::lsp_position_to_byte(source, edit.range.start);
            let end = falcon_diagnostics::lsp_position_to_byte(source, edit.range.end);
            result.replace_range(start..end, &edit.new_text);
        }
        return result;
    }
    assert!(edit.changes.is_none(), "edits must be versioned");
    let Some(lsp_types::DocumentChanges::Edits(documents)) = &edit.document_changes else {
        panic!("expected versioned document edits");
    };
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].text_document.version, Some(version));
    assert_eq!(documents[0].text_document.uri.as_str(), DOC_URI);
    let mut result = source.to_owned();
    for edit in documents[0].edits.iter().rev() {
        let lsp_types::OneOf::Left(edit) = edit else {
            panic!("expected text edit");
        };
        let start = falcon_diagnostics::lsp_position_to_byte(source, edit.range.start);
        let end = falcon_diagnostics::lsp_position_to_byte(source, edit.range.end);
        result.replace_range(start..end, &edit.new_text);
    }
    result
}

fn executed_action_text(
    client: &mut TestClient,
    action: &lsp_types::CodeAction,
    version: i32,
    source: &str,
) -> String {
    assert!(
        action.edit.is_none(),
        "listing fix-all must not compute edits"
    );
    let command = action.command.as_ref().expect("fix-all has a command");
    assert_eq!(command.command, "falcon.fixAll");
    let id = client.request("workspace/executeCommand", command);
    let Message::Request(apply) = client.recv() else {
        panic!("executing fix-all must request workspace/applyEdit");
    };
    assert_eq!(apply.method, "workspace/applyEdit");
    let params: lsp_types::ApplyWorkspaceEditParams = serde_json::from_value(apply.params).unwrap();
    let result = action_text(
        &lsp_types::CodeAction {
            edit: Some(params.edit),
            ..Default::default()
        },
        version,
        source,
    );
    client
        .client
        .sender
        .send(
            lsp_server::Response::new_ok(
                apply.id,
                lsp_types::ApplyWorkspaceEditResponse {
                    applied: true,
                    failure_reason: None,
                    failed_change: None,
                },
            )
            .into(),
        )
        .unwrap();
    let response = client.recv_response(&id);
    assert!(response.error.is_none(), "{response:?}");
    result
}

const FIX_SOURCE: &str = "void main() {\n  final x = new Object();\n  print(\"hello\");\n}\n";
const FIX_CONFIG: &str = r#"{"linter":{"rules":{"recommended":false,"style":{
    "unnecessary-new":"warn",
    "prefer-single-quotes":{"level":"warn","fix":"unsafe"}
}}}}"#;

fn full_range() -> lsp_types::Range {
    lsp_types::Range::new(Position::new(0, 0), Position::new(20, 0))
}

#[test]
fn quick_fixes_honor_context_range_and_effective_safety() {
    let mut client = TestClient::start(Duration::ZERO, Some(FIX_CONFIG));
    client.open(DOC_URI, FIX_SOURCE, 7);
    let published = client.recv_publish();
    let actions = code_actions(
        &mut client,
        published.diagnostics.clone(),
        Some(vec!["quickfix"]),
        full_range(),
    );
    assert_eq!(actions.len(), 2, "{actions:?}");
    let safe = actions
        .iter()
        .find(|action| action.is_preferred == Some(true))
        .unwrap();
    assert!(!safe.title.contains("unsafe"));
    assert!(
        safe.diagnostics
            .as_ref()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.code
                == Some(lsp_types::NumberOrString::String("unnecessary-new".into())))
    );
    assert_eq!(
        action_text(safe, 7, FIX_SOURCE),
        FIX_SOURCE.replace("new ", "")
    );
    let unsafe_action = actions
        .iter()
        .find(|action| action.is_preferred == Some(false))
        .unwrap();
    assert!(unsafe_action.title.contains("unsafe"));
    assert_eq!(
        action_text(unsafe_action, 7, FIX_SOURCE),
        FIX_SOURCE.replace("\"hello\"", "'hello'")
    );

    let quote = published
        .diagnostics
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(lsp_types::NumberOrString::String(
                    "prefer-single-quotes".into(),
                ))
        })
        .unwrap();
    let selected = code_actions(
        &mut client,
        vec![quote.clone()],
        Some(vec!["quickfix"]),
        quote.range,
    );
    assert_eq!(selected.len(), 1);
    assert!(selected[0].title.contains("unsafe"));
    let outside = code_actions(
        &mut client,
        vec![quote],
        Some(vec!["quickfix"]),
        lsp_types::Range::new(Position::new(0, 0), Position::new(0, 0)),
    );
    assert!(outside.is_empty());
    client.shutdown();
}

#[test]
fn generic_fix_all_is_safe_and_unsafe_requires_separate_kind() {
    let mut client = TestClient::start(Duration::ZERO, Some(FIX_CONFIG));
    client.open(DOC_URI, FIX_SOURCE, 3);
    client.recv_publish();
    let safe = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["source.fixAll"]),
        full_range(),
    );
    assert_eq!(safe.len(), 1);
    assert_eq!(
        safe[0].kind.as_ref().unwrap().as_str(),
        "source.fixAll.falcon"
    );
    let safe_text = executed_action_text(&mut client, &safe[0], 3, FIX_SOURCE);
    assert!(!safe_text.contains("new Object"));
    assert!(safe_text.contains("\"hello\""), "{safe_text}");
    let unsafe_actions = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["source.falcon.fixAllUnsafe"]),
        full_range(),
    );
    assert_eq!(unsafe_actions.len(), 1);
    let unsafe_text = executed_action_text(&mut client, &unsafe_actions[0], 3, FIX_SOURCE);
    assert!(!unsafe_text.contains("new Object"));
    assert!(unsafe_text.contains("'hello'"), "{unsafe_text}");
    let unsupported = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["refactor"]),
        full_range(),
    );
    assert!(unsupported.is_empty());
    client.shutdown();
}

#[test]
fn disabled_fix_policy_keeps_diagnostics_without_actions() {
    let config = FIX_CONFIG
        .replace("\"fix\":\"unsafe\"", "\"fix\":\"none\"")
        .replace(
            "\"unnecessary-new\":\"warn\"",
            "\"unnecessary-new\":{\"level\":\"warn\",\"fix\":\"none\"}",
        );
    let mut client = TestClient::start(Duration::ZERO, Some(&config));
    client.open(DOC_URI, FIX_SOURCE, 1);
    let published = client.recv_publish();
    assert!(has_rule(&published, "unnecessary-new"));
    assert!(has_rule(&published, "prefer-single-quotes"));
    assert!(code_actions(&mut client, published.diagnostics, None, full_range()).is_empty());
    client.shutdown();
}

#[test]
fn code_actions_wait_for_pending_buffer_publication() {
    let mut client = TestClient::start(Duration::from_secs(60), Some(FIX_CONFIG));
    client.open(DOC_URI, FIX_SOURCE, 1);
    let published = client.recv_publish();
    let current = format!("\n{FIX_SOURCE}");
    client.change(DOC_URI, &current, 2);
    let stale = code_actions(
        &mut client,
        published.diagnostics,
        Some(vec!["quickfix"]),
        full_range(),
    );
    assert!(
        stale.is_empty(),
        "old diagnostic spans must not produce fixes"
    );
    let safe = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["source.fixAll.falcon"]),
        full_range(),
    );
    assert!(
        safe.is_empty(),
        "fix-all must also wait for diagnostics publication"
    );
    client.shutdown();
}

#[test]
fn cyclic_fix_all_returns_error_instead_of_partial_edits() {
    let config = r#"{"linter":{"rules":{"recommended":false,"style":{
        "prefer-single-quotes":"warn","prefer-double-quotes":"warn"
    }}}}"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, "void main() { print(\"hello\"); }\n", 1);
    client.recv_publish();
    // Merely opening a lightbulb must still return an individual quick fix,
    // even though explicitly applying all fixes would cycle.
    let quick = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["quickfix"]),
        full_range(),
    );
    assert_eq!(quick.len(), 1);
    assert_eq!(quick[0].kind, Some(lsp_types::CodeActionKind::QUICKFIX));
    assert_eq!(
        action_text(&quick[0], 1, "void main() { print(\"hello\"); }\n"),
        "void main() { print('hello'); }\n"
    );
    let mixed = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["quickfix", "source.fixAll.falcon"]),
        full_range(),
    );
    assert_eq!(mixed.len(), 2);
    let quickfix = mixed
        .iter()
        .find(|action| action.kind == Some(lsp_types::CodeActionKind::QUICKFIX))
        .unwrap();
    assert!(quickfix.disabled.is_none());
    assert_eq!(
        action_text(quickfix, 1, "void main() { print(\"hello\"); }\n"),
        "void main() { print('hello'); }\n"
    );
    let stopped = mixed
        .iter()
        .find(|action| {
            action
                .kind
                .as_ref()
                .is_some_and(|kind| kind.as_str() == "source.fixAll.falcon")
        })
        .unwrap();
    assert!(stopped.edit.is_none());
    assert!(
        stopped.disabled.is_none(),
        "listing must not run cyclic fixes"
    );
    assert!(stopped.command.is_some());
    let mut only = code_actions(
        &mut client,
        Vec::new(),
        Some(vec!["source.fixAll.falcon"]),
        full_range(),
    );
    assert_eq!(only.len(), 1);
    for action in [stopped.clone(), only.pop().unwrap()] {
        assert!(action.edit.is_none());
        assert!(action.disabled.is_none());
        let id = client.request(
            "workspace/executeCommand",
            action.command.as_ref().expect("lazy fix-all command"),
        );
        // An applyEdit request here would fail recv_response: cycles must
        // return an execution error without exposing any partial edits.
        let response = client.recv_response(&id);
        assert!(response.result.is_none(), "{response:?}");
        let error = response.error.expect("cycle must be reported on execution");
        assert!(
            error.message.contains("without returning edits"),
            "{error:?}"
        );
        assert!(error.message.contains("prefer-"), "{error:?}");
    }
    client.shutdown();
}

/// Initialize/initialized/shutdown cycle with shape validation.
#[test]
fn initialize_shutdown_cycle() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.shutdown();
}

#[test]
fn config_warnings_reach_editor_on_startup_and_watched_reload_without_open_documents() {
    let mut client = TestClient::start(Duration::ZERO, Some(r#"{"max_errors": 10}"#));
    let receive_message = |client: &TestClient| {
        let Message::Notification(notification) = client.recv() else {
            panic!("expected configuration notification");
        };
        assert_eq!(
            notification.method,
            lsp_types::notification::ShowMessage::METHOD
        );
        serde_json::from_value::<lsp_types::ShowMessageParams>(notification.params).unwrap()
    };
    let warning = receive_message(&client);
    assert_eq!(warning.typ, lsp_types::MessageType::WARNING);
    assert!(warning.message.contains("max_errors"), "{warning:?}");
    assert!(
        warning
            .message
            .contains(&client.config_path().display().to_string())
    );
    client.assert_quiet(Duration::from_millis(50));

    fs::write(
        client.config_path(),
        r#"{"linter": {"domains": {"fluter": "on"}}}"#,
    )
    .unwrap();
    client.notify(
        lsp_types::notification::DidChangeWatchedFiles::METHOD,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Uri::from_str(&format!("file://{}", client.config_path().display())).unwrap(),
                typ: FileChangeType::CHANGED,
            }],
        },
    );
    let error = receive_message(&client);
    assert_eq!(error.typ, lsp_types::MessageType::ERROR);
    assert!(error.message.contains("linter.domains.fluter"), "{error:?}");
    assert!(
        error
            .message
            .contains(&client.config_path().display().to_string())
    );
    client.assert_quiet(Duration::from_millis(50));

    fs::write(client.config_path(), r#"{"max_errors": 20}"#).unwrap();
    client.notify(
        lsp_types::notification::DidSaveTextDocument::METHOD,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(&format!("file://{}", client.config_path().display())).unwrap(),
            },
            text: None,
        },
    );
    let warning = receive_message(&client);
    assert_eq!(warning.typ, lsp_types::MessageType::WARNING);
    assert!(warning.message.contains("max_errors"), "{warning:?}");
    client.assert_quiet(Duration::from_millis(50));
    let nested = client
        .config_path()
        .parent()
        .unwrap()
        .join("nested/falcon.json");
    fs::create_dir_all(nested.parent().unwrap()).unwrap();
    fs::write(&nested, "{}").unwrap();
    for path in [client.config_path(), nested] {
        client.notify(
            lsp_types::notification::DidChangeWatchedFiles::METHOD,
            DidChangeWatchedFilesParams {
                changes: vec![FileEvent {
                    uri: Uri::from_str(&format!("file://{}", path.display())).unwrap(),
                    typ: FileChangeType::CHANGED,
                }],
            },
        );
        client.assert_quiet(Duration::from_millis(50));
    }
    client.shutdown();
}

#[test]
fn config_warnings_are_batched_and_unchanged_reload_is_quiet() {
    let mut client = TestClient::start(
        Duration::ZERO,
        Some(r#"{"max_errors": 10, "cross_file": {"enabled": false}}"#),
    );
    let Message::Notification(notification) = client.recv() else {
        panic!("expected configuration notification");
    };
    assert_eq!(
        notification.method,
        lsp_types::notification::ShowMessage::METHOD
    );
    let warning =
        serde_json::from_value::<lsp_types::ShowMessageParams>(notification.params).unwrap();
    assert_eq!(warning.typ, lsp_types::MessageType::WARNING);
    let lines: Vec<_> = warning.message.lines().collect();
    assert_eq!(lines[0], "Falcon: 2 configuration warnings");
    assert_eq!(lines.len(), 3, "{warning:?}");
    assert!(lines[1..].iter().any(|line| line.contains("max_errors")));
    assert!(lines[1..].iter().any(|line| line.contains("cross_file")));
    for line in &lines[1..] {
        assert!(line.contains(&client.config_path().display().to_string()));
    }
    client.assert_quiet(Duration::from_millis(50));

    client.notify(
        lsp_types::notification::DidChangeWatchedFiles::METHOD,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Uri::from_str(&format!("file://{}", client.config_path().display())).unwrap(),
                typ: FileChangeType::CHANGED,
            }],
        },
    );
    client.assert_quiet(Duration::from_millis(50));
    client.shutdown();
}

#[test]
fn generated_empty_uri_self_import_does_not_show_configuration_warning() {
    let mut client = TestClient::start(Duration::ZERO, None);
    let path = client
        .config_path()
        .parent()
        .unwrap()
        .join("generated.dart");
    let source = "import '' as self;\nclass Generated {}\nself.Generated? binding;\n";
    fs::write(&path, source).unwrap();
    let uri = format!("file://{}", path.display());
    client.open(&uri, source, 1);
    let params = client.recv_publish();
    assert_eq!(params.uri.as_str(), uri);
    assert_eq!(params.version, Some(1));
    client.assert_quiet(Duration::from_millis(50));
    client.shutdown();
}

/// didOpen publishes diagnostics for a violating file.
#[test]
fn did_open_publishes_diagnostics() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    let params = client.recv_publish();
    assert_eq!(params.uri.as_str(), DOC_URI);
    assert_eq!(params.version, Some(1));
    assert!(has_rule(&params, "avoid-dynamic"), "{params:?}");
    let diag = params
        .diagnostics
        .iter()
        .find(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(c)) if c == "avoid-dynamic"))
        .unwrap();
    assert_eq!(diag.source.as_deref(), Some("falcon"));
    assert_eq!(diag.range.start.line, 1, "dynamic is on line 2 (0-based 1)");
    client.shutdown();
}

/// didChange re-analyzes and republishes; fixing the violation
/// clears the diagnostic; the version is echoed.
#[test]
fn did_change_republishes() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    assert!(has_rule(&client.recv_publish(), "avoid-dynamic"));

    client.change(DOC_URI, CLEAN_SRC, 2);
    let params = client.recv_publish();
    assert_eq!(params.version, Some(2));
    assert!(
        !has_rule(&params, "avoid-dynamic"),
        "fixed file must not re-report: {params:?}"
    );
    client.shutdown();
}

#[test]
fn did_change_republishes_cross_file_diagnostics() {
    let config = r#"{
        "cross-file": { "rules": {
            "recommended": false,
            "correctness": { "unused-code": "warn" }
        } }
    }"#;

    for debounce in [Duration::ZERO, Duration::from_millis(25)] {
        let mut client = TestClient::start(debounce, Some(config));
        let path = client.config_path().parent().unwrap().join("unused.dart");
        let uri = format!("file://{}", path.display());
        let source = "void unused() {}\n";
        fs::write(&path, source).unwrap();

        client.open(&uri, source, 1);
        let initial = client.recv_publish();
        assert!(
            has_rule(&initial, "unused-code"),
            "open must publish the cross-file diagnostic: {initial:?}"
        );
        client.assert_quiet(Duration::from_millis(50));

        client.change(&uri, "void unused() {}\n\n", 2);
        let refreshed = client.recv_publish();
        assert_eq!(refreshed.version, Some(2));
        assert!(
            has_rule(&refreshed, "unused-code"),
            "didChange must republish the cross-file diagnostic with debounce {debounce:?}: {refreshed:?}"
        );
        client.assert_quiet(Duration::from_millis(50));
        client.shutdown();
    }
}

/// Resolver-backed diagnostics in open importers refresh when an exported
/// signature changes in another open document.
#[test]
fn did_change_republishes_resolver_backed_diagnostics_in_dependents() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let api_path = dir.join("api.dart");
    let consumer_path = dir.join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let returning = "int calculate() => 1;\n";
    let returning_void = "void calculate() {}\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, returning).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    client.open(&api_uri, returning, 1);
    client.recv_publish();
    client.open(&consumer_uri, consumer, 1);
    let initial = client.recv_publish();
    assert!(
        has_rule(&initial, "avoid-ignoring-return-values"),
        "imported non-void call must initially report: {initial:?}"
    );

    client.change(&api_uri, returning_void, 2);
    let first = client.recv_publish();
    let second = client.recv_publish();
    let dependent = [&first, &second]
        .into_iter()
        .find(|params| params.uri.as_str() == consumer_uri)
        .expect("dependent document must be republished");
    assert!(
        !has_rule(dependent, "avoid-ignoring-return-values"),
        "changing the imported return type to void must clear the dependent diagnostic: {dependent:?}"
    );
    client.shutdown();
}

#[test]
fn debounced_related_changes_publish_each_affected_document_once() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    // Debounce long enough that both changes reliably land in one flush.
    let mut client = TestClient::start(Duration::from_millis(150), Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let api_path = dir.join("api.dart");
    let consumer_path = dir.join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let api = "int calculate() => 1;\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, api).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    client.open(&api_uri, api, 1);
    client.recv_publish();
    client.open(&consumer_uri, consumer, 1);
    client.recv_publish();

    client.change(&api_uri, "void calculate() {}\n", 2);
    client.change(&consumer_uri, &format!("{consumer}\n"), 2);
    let mut published = vec![
        client.recv_publish().uri.as_str().to_string(),
        client.recv_publish().uri.as_str().to_string(),
    ];
    published.sort();
    assert_eq!(published, vec![api_uri, consumer_uri]);
    client.assert_quiet(Duration::from_millis(300));
    client.shutdown();
}

#[test]
fn did_open_refreshes_disk_and_buffer_part_owners() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let old_path = dir.join("old.dart");
    let new_path = dir.join("new.dart");
    let pivot_path = dir.join("pivot.dart");
    let old_uri = format!("file://{}", old_path.display());
    let new_uri = format!("file://{}", new_path.display());
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&old_path, "library old;\n").unwrap();
    fs::write(&new_path, "library new;\n").unwrap();
    fs::write(&pivot_path, "part of 'old.dart';\n").unwrap();

    client.open(&old_uri, "library old;\n", 1);
    client.recv_publish();
    client.open(&new_uri, "library new;\n", 1);
    client.recv_publish();
    client.open(&pivot_uri, "part of 'new.dart';\n", 1);

    let mut published = (0..3)
        .map(|_| client.recv_publish().uri.as_str().to_string())
        .collect::<Vec<_>>();
    published.sort();
    assert_eq!(published, vec![new_uri, old_uri, pivot_uri]);
    client.shutdown();
}

/// Percent-encoded file URIs map to the same filesystem paths used by the
/// workspace walk, so imported declarations still resolve.
#[test]
fn percent_encoded_file_uris_resolve_workspace_dependencies() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let dir = client.config_path().parent().unwrap().join("with space");
    fs::create_dir(&dir).unwrap();
    let api_path = dir.join("api.dart");
    let consumer_path = dir.join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display()).replace(' ', "%20");
    let consumer_uri = format!("file://{}", consumer_path.display()).replace(' ', "%20");
    let api = "int calculate() => 1;\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, api).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    client.open(&api_uri, api, 1);
    client.recv_publish();
    client.open(&consumer_uri, consumer, 1);
    let diagnostics = client.recv_publish();
    assert!(
        has_rule(&diagnostics, "avoid-ignoring-return-values"),
        "encoded URI paths must bind the imported declaration: {diagnostics:?}"
    );
    client.shutdown();
}

/// Saving before a pending debounce fires still immediately refreshes resolver-
/// backed diagnostics in open importers.
#[test]
fn did_save_immediately_refreshes_resolver_backed_diagnostics_in_dependents() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::from_secs(30), Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let api_path = dir.join("api.dart");
    let consumer_path = dir.join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let returning = "int calculate() => 1;\n";
    let returning_void = "void calculate() {}\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, returning).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    client.open(&api_uri, returning, 1);
    client.recv_publish();
    client.open(&consumer_uri, consumer, 1);
    let initial = client.recv_publish();
    assert!(
        has_rule(&initial, "avoid-ignoring-return-values"),
        "the imported non-void call must initially report: {initial:?}"
    );

    client.change(&api_uri, returning_void, 2);
    client.notify(
        lsp_types::notification::DidSaveTextDocument::METHOD,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(&api_uri).unwrap(),
            },
            text: Some(returning_void.to_string()),
        },
    );
    let first = client.recv_publish();
    let second = client.recv_publish();
    let dependent = [&first, &second]
        .into_iter()
        .find(|params| params.uri.as_str() == consumer_uri)
        .expect("immediate save must republish the open importer");
    assert!(
        !has_rule(dependent, "avoid-ignoring-return-values"),
        "saving the void signature must immediately clear the dependent diagnostic: {dependent:?}"
    );

    client.notify(
        lsp_types::notification::DidSaveTextDocument::METHOD,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(&api_uri).unwrap(),
            },
            text: Some(returning.to_string()),
        },
    );
    let first = client.recv_publish();
    let second = client.recv_publish();
    let dependent = [&first, &second]
        .into_iter()
        .find(|params| params.uri.as_str() == consumer_uri)
        .expect("save text without didChange must republish the open importer");
    assert!(
        has_rule(dependent, "avoid-ignoring-return-values"),
        "save text without a change event must update dependent diagnostics: {dependent:?}"
    );
    client.shutdown();
}

/// Closing an imported library refreshes open importers against the on-disk
/// version after first clearing diagnostics for the closed URI.
#[test]
fn did_close_refreshes_resolver_backed_diagnostics_in_dependents() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let api_path = dir.join("api.dart");
    let consumer_path = dir.join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let returning_void = "void calculate() {}\n";
    let returning = "int calculate() => 1;\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, returning_void).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    client.open(&consumer_uri, consumer, 1);
    assert!(!has_rule(
        &client.recv_publish(),
        "avoid-ignoring-return-values"
    ));
    client.open(&api_uri, returning, 1);
    let first = client.recv_publish();
    let second = client.recv_publish();
    let dependent = [&first, &second]
        .into_iter()
        .find(|params| params.uri.as_str() == consumer_uri)
        .expect("opening the dependency must republish the importer");
    assert!(
        has_rule(dependent, "avoid-ignoring-return-values"),
        "the open non-void signature must add the dependent diagnostic: {dependent:?}"
    );

    client.notify(
        lsp_types::notification::DidCloseTextDocument::METHOD,
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(&api_uri).unwrap(),
            },
        },
    );
    let cleared = client.recv_publish();
    assert_eq!(cleared.uri.as_str(), api_uri);
    assert!(
        cleared.diagnostics.is_empty(),
        "the closed URI must be cleared first: {cleared:?}"
    );
    let dependent = client.recv_publish();
    assert_eq!(dependent.uri.as_str(), consumer_uri);
    assert!(
        !has_rule(&dependent, "avoid-ignoring-return-values"),
        "falling back to the on-disk void signature must clear the dependent diagnostic: {dependent:?}"
    );
    client.shutdown();
}

#[test]
fn did_close_refreshes_buffer_and_restored_disk_part_owners() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let dir = client.config_path().parent().unwrap().to_path_buf();
    let old_path = dir.join("old.dart");
    let new_path = dir.join("new.dart");
    let pivot_path = dir.join("pivot.dart");
    let old_uri = format!("file://{}", old_path.display());
    let new_uri = format!("file://{}", new_path.display());
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&old_path, "library old;\n").unwrap();
    fs::write(&new_path, "library new;\n").unwrap();
    fs::write(&pivot_path, "part of 'new.dart';\n").unwrap();

    client.open(&old_uri, "library old;\n", 1);
    client.recv_publish();
    client.open(&new_uri, "library new;\n", 1);
    client.recv_publish();
    client.open(&pivot_uri, "part of 'old.dart';\n", 1);
    for _ in 0..3 {
        client.recv_publish();
    }

    client.notify(
        lsp_types::notification::DidCloseTextDocument::METHOD,
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(&pivot_uri).unwrap(),
            },
        },
    );
    let cleared = client.recv_publish();
    assert_eq!(cleared.uri.as_str(), pivot_uri);
    assert!(cleared.diagnostics.is_empty());
    let mut refreshed = (0..2)
        .map(|_| client.recv_publish().uri.as_str().to_string())
        .collect::<Vec<_>>();
    refreshed.sort();
    assert_eq!(refreshed, vec![new_uri, old_uri]);
    client.shutdown();
}

/// didSave re-analyzes (with included text refreshing the cache).
#[test]
fn did_save_republishes() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, CLEAN_SRC, 1);
    client.recv_publish();

    client.notify(
        lsp_types::notification::DidSaveTextDocument::METHOD,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(DOC_URI).unwrap(),
            },
            text: Some(VIOLATING_SRC.to_string()),
        },
    );
    let params = client.recv_publish();
    assert!(has_rule(&params, "avoid-dynamic"), "{params:?}");
    client.shutdown();
}

/// didClose publishes an empty diagnostic set to clear the editor.
#[test]
fn did_close_clears_diagnostics() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    assert!(!client.recv_publish().diagnostics.is_empty());

    client.notify(
        lsp_types::notification::DidCloseTextDocument::METHOD,
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(DOC_URI).unwrap(),
            },
        },
    );
    let params = client.recv_publish();
    assert!(params.diagnostics.is_empty());
    client.shutdown();
}

/// Hover over a diagnostic returns its rule and message; hover over
/// clean code returns null.
#[test]
fn hover_reports_diagnostics_under_cursor() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    client.recv_publish();

    let hover_at = |client: &mut TestClient, line: u32, character: u32| -> Option<Hover> {
        let id = client.request(
            lsp_types::request::HoverRequest::METHOD,
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: Uri::from_str(DOC_URI).unwrap(),
                    },
                    position: Position { line, character },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        );
        let resp = client.recv_response(&id);
        serde_json::from_value(
            resp.result
                .expect("hover must return a result (possibly null)"),
        )
        .expect("Hover shape")
    };

    // Line 2 (0-based 1) holds `dynamic x = 1;`.
    let hover = hover_at(&mut client, 1, 3).expect("hover over diagnostic");
    match hover.contents {
        lsp_types::HoverContents::Markup(content) => {
            assert!(content.value.contains("avoid-dynamic"), "{}", content.value)
        }
        other => panic!("expected markup hover, got {other:?}"),
    }

    // Line 1 (0-based 0) is the clean function signature.
    assert!(hover_at(&mut client, 0, 0).is_none());
    client.shutdown();
}

/// Unknown request methods get a MethodNotFound error response.
#[test]
fn unsupported_request_returns_method_not_found() {
    let mut client = TestClient::start(Duration::ZERO, None);
    let id = client.request("textDocument/definition", serde_json::json!({}));
    let resp = client.recv_response(&id);
    let err = resp.error.expect("must be an error response");
    assert_eq!(err.code, lsp_server::ErrorCode::MethodNotFound as i32);
    client.shutdown();
}

/// Rules disabled in falcon.json never fire in LSP mode.
#[test]
fn config_disabled_rule_not_published() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    assert!(has_rule(&client.recv_publish(), "avoid-dynamic"));

    fs::write(
        client.config_path(),
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }"#,
    )
    .unwrap();
    let config_uri = format!("file://{}", client.config_path().display());
    client.notify(
        lsp_types::notification::DidChangeWatchedFiles::METHOD,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Uri::from_str(&config_uri).unwrap(),
                typ: FileChangeType::CHANGED,
            }],
        },
    );
    let params = client.recv_publish();
    assert!(
        !has_rule(&params, "avoid-dynamic"),
        "disabled rule fired: {params:?}"
    );
    client.shutdown();
}

/// A configured rule level flows through to published diagnostic severity.
#[test]
fn config_rule_level_sets_published_severity() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "info" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    let params = client.recv_publish();
    let diag = params
        .diagnostics
        .iter()
        .find(|d| {
            matches!(&d.code, Some(lsp_types::NumberOrString::String(code)) if code == "avoid-dynamic")
        })
        .expect("avoid-dynamic must be published");
    assert_eq!(
        diag.severity,
        Some(lsp_types::DiagnosticSeverity::INFORMATION)
    );
    client.shutdown();
}

/// workspace/didChangeWatchedFiles reloads falcon.json and republishes
/// every open document against its cached CST.
#[test]
fn watched_files_change_reloads_config() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
    assert!(has_rule(&client.recv_publish(), "avoid-dynamic"));

    // Disable the rule on disk, then tell the server the file changed.
    fs::write(
        client.config_path(),
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }"#,
    )
    .unwrap();
    let config_uri = format!("file://{}", client.config_path().display());
    client.notify(
        lsp_types::notification::DidChangeWatchedFiles::METHOD,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Uri::from_str(&config_uri).unwrap(),
                typ: FileChangeType::CHANGED,
            }],
        },
    );
    let params = client.recv_publish();
    assert_eq!(params.uri.as_str(), DOC_URI);
    assert!(
        !has_rule(&params, "avoid-dynamic"),
        "rule must be gone after config reload: {params:?}"
    );
    client.shutdown();
}

/// Rapid didChange events are debounced into one
/// analysis/publish reflecting the final text.
#[test]
fn debounce_coalesces_rapid_changes() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::from_millis(150), Some(config));
    client.open(DOC_URI, CLEAN_SRC, 1);
    client.recv_publish();

    client.change(DOC_URI, "void f() {}\n", 2);
    client.change(DOC_URI, "void f() { print(1); }\n", 3);
    client.change(DOC_URI, VIOLATING_SRC, 4);

    // Exactly one publish arrives (after the quiet window), for the last text.
    let params = client.recv_publish();
    assert_eq!(params.version, Some(4));
    assert!(has_rule(&params, "avoid-dynamic"), "{params:?}");
    client.assert_quiet(Duration::from_millis(300));
    client.shutdown();
}

/// An override that disables a rule for the document's path filters its
/// diagnostics in LSP mode, just like the CLI.
#[test]
fn config_override_disables_rule_not_published() {
    // Relative override globs are rooted at the config directory.
    let config = r#"{
        "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } },
        "overrides": [ {
            "includes": ["**/a.dart"],
            "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } }
        } ]
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    let root = client.config_path().parent().unwrap().to_path_buf();
    let baseline_uri = format!("file://{}", root.join("b.dart").display());
    let overridden_uri = format!("file://{}", root.join("a.dart").display());
    client.open(&baseline_uri, VIOLATING_SRC, 1);
    let baseline = client.recv_publish();
    assert_eq!(baseline.uri.as_str(), baseline_uri);
    assert!(has_rule(&baseline, "avoid-dynamic"), "{baseline:?}");

    client.open(&overridden_uri, VIOLATING_SRC, 1);
    let params = client.recv_publish();
    assert!(
        !has_rule(&params, "avoid-dynamic"),
        "override-disabled rule fired: {params:?}"
    );
    client.shutdown();
}

/// Non-compiling Dart surfaces `syntax-error` diagnostics over LSP, matching the
/// CLI — otherwise broken code is silently linted as if it parsed.
#[test]
fn did_open_publishes_syntax_error_diagnostics() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.open(DOC_URI, SYNTAX_ERR_SRC, 1);
    let params = client.recv_publish();
    assert!(
        has_rule(&params, "syntax-error"),
        "syntax errors must be published over LSP: {params:?}"
    );
    let diag = params
        .diagnostics
        .iter()
        .find(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(c)) if c == "syntax-error"))
        .unwrap();
    assert_eq!(diag.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    client.shutdown();
}

/// Published columns count UTF-16 code units: a syntax error after two emoji
/// lands at character 32 (2 emoji × 2 units), not 30 (scalar count).
#[test]
fn published_positions_count_utf16_code_units() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.open(DOC_URI, EMOJI_SYNTAX_SRC, 1);
    let params = client.recv_publish();
    let diag = params
        .diagnostics
        .iter()
        .find(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(c)) if c == "syntax-error"))
        .expect("syntax error must be published");
    assert_eq!(diag.range.start.line, 0);
    assert_eq!(
        diag.range.start.character, 32,
        "column must count UTF-16 units past the emoji, not scalars: {:?}",
        diag.range
    );
    client.shutdown();
}

/// A single malformed notification (didChange with `version: null`, a real
/// client quirk) must be ignored, not tear the session down. The server stays
/// alive and still analyzes a subsequent well-formed change.
#[test]
fn malformed_did_change_params_do_not_kill_server() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, CLEAN_SRC, 1);
    client.recv_publish();

    // `version: null` violates VersionedTextDocumentIdentifier (required i32).
    client.notify(
        lsp_types::notification::DidChangeTextDocument::METHOD,
        serde_json::json!({
            "textDocument": { "uri": DOC_URI, "version": null },
            "contentChanges": [ { "text": VIOLATING_SRC } ]
        }),
    );

    // Ignored, not fatal: the next valid change is still analyzed and published.
    client.change(DOC_URI, VIOLATING_SRC, 3);
    let params = client.recv_publish();
    assert_eq!(params.version, Some(3));
    assert!(
        has_rule(&params, "avoid-dynamic"),
        "server must survive malformed params: {params:?}"
    );
    client.shutdown();
}

/// A malformed *request* gets an InvalidParams error response rather than
/// killing the server (which then keeps handling messages).
#[test]
fn malformed_request_params_return_invalid_params() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.open(DOC_URI, VIOLATING_SRC, 1);
    client.recv_publish();

    // position.line as a string is not a valid HoverParams.
    let id = client.request(
        lsp_types::request::HoverRequest::METHOD,
        serde_json::json!({
            "textDocument": { "uri": DOC_URI },
            "position": { "line": "nope", "character": 0 }
        }),
    );
    let resp = client.recv_response(&id);
    let err = resp.error.expect("malformed request must error");
    assert_eq!(err.code, lsp_server::ErrorCode::InvalidParams as i32);
    client.shutdown();
}

/// `exit` without a prior `shutdown` must fail the server loop (process exit
/// code 1 per the LSP spec), not return success.
#[test]
fn exit_without_shutdown_terminates_with_error() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.notify(lsp_types::notification::Exit::METHOD, ());
    let server = client.server.take().unwrap();
    let result = server.join();
    assert!(
        result.is_err(),
        "exit before shutdown must fail the server loop (exit code 1)"
    );
}

#[test]
fn unrelated_open_save_and_close_publish_while_another_file_is_debounced() {
    let config = r#"{
        "linter": { "rules": { "recommended": false } },
        "cross-file": { "enabled": false }
    }"#;
    // Deliberately longer than the receive deadline: an accidental dependency
    // on the dirty file's debounce will fail instead of eventually succeeding.
    let mut client = TestClient::start(Duration::from_secs(60), Some(config));
    let dirty = "file:///test/dirty.dart";
    let other = "file:///test/other.dart";
    client.open(dirty, "void before() {}", 1);
    assert_eq!(client.recv_publish().uri.as_str(), dirty);
    client.change(dirty, "void after() {}", 2);

    client.open(other, "void other() {}", 1);
    let opened = client.recv_publish();
    assert_eq!(opened.uri.as_str(), other);
    assert_eq!(opened.version, Some(1));
    client.notify(
        lsp_types::notification::DidSaveTextDocument::METHOD,
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(other).unwrap(),
            },
            text: None,
        },
    );
    assert_eq!(client.recv_publish().uri.as_str(), other);
    client.notify(
        lsp_types::notification::DidCloseTextDocument::METHOD,
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(other).unwrap(),
            },
        },
    );
    let closed = client.recv_publish();
    assert_eq!(closed.uri.as_str(), other);
    assert!(closed.diagnostics.is_empty());
    client.shutdown();
}
