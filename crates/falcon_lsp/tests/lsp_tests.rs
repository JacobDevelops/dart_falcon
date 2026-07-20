//! LSP protocol tests (M5.1): a mock client drives the real server loop over
//! `Connection::memory()`, so the exact production code path is exercised.
//!
//! JSON-RPC 2.0 / LSP 3.17 compliance is checked the way the plan specifies:
//! every server message is deserialized back into the corresponding
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
    /// (every rule enabled) for hermeticity.
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
            lsp_types::InitializeParams::default(),
        );
        let resp = self.recv_response(&id);
        // Plan M5.1: validate the message shape against lsp_types.
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

/// M5.1: initialize/initialized/shutdown cycle with shape validation.
#[test]
fn initialize_shutdown_cycle() {
    let mut client = TestClient::start(Duration::ZERO, None);
    client.shutdown();
}

/// M5.1: didOpen publishes diagnostics for a violating file.
#[test]
fn did_open_publishes_diagnostics() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.1/M5.2: didChange re-analyzes and republishes; fixing the violation
/// clears the diagnostic; the version is echoed.
#[test]
fn did_change_republishes() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.1: didSave re-analyzes (with included text refreshing the cache).
#[test]
fn did_save_republishes() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.1: didClose publishes an empty diagnostic set to clear the editor.
#[test]
fn did_close_clears_diagnostics() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.1: hover over a diagnostic returns its rule and message; hover over
/// clean code returns null.
#[test]
fn hover_reports_diagnostics_under_cursor() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.1: unknown request methods get a MethodNotFound error response.
#[test]
fn unsupported_request_returns_method_not_found() {
    let mut client = TestClient::start(Duration::ZERO, None);
    let id = client.request("textDocument/definition", serde_json::json!({}));
    let resp = client.recv_response(&id);
    let err = resp.error.expect("must be an error response");
    assert_eq!(err.code, lsp_server::ErrorCode::MethodNotFound as i32);
    client.shutdown();
}

/// M5.2: rules disabled in falcon.json never fire in LSP mode.
#[test]
fn config_disabled_rule_not_published() {
    let config = r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
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

/// M5.2: workspace/didChangeWatchedFiles reloads falcon.json and republishes
/// every open document against its cached AST.
#[test]
fn watched_files_change_reloads_config() {
    let mut client = TestClient::start(Duration::ZERO, None);
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

/// M5.2 (design §4): rapid didChange events are debounced into one
/// analysis/publish reflecting the final text.
#[test]
fn debounce_coalesces_rapid_changes() {
    let mut client = TestClient::start(Duration::from_millis(150), None);
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
    // DOC_URI resolves to `/test/a.dart`; `**/a.dart` matches that walked path.
    let config = r#"{
        "overrides": [ {
            "includes": ["**/a.dart"],
            "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } }
        } ]
    }"#;
    let mut client = TestClient::start(Duration::ZERO, Some(config));
    client.open(DOC_URI, VIOLATING_SRC, 1);
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
    let mut client = TestClient::start(Duration::ZERO, None);
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
