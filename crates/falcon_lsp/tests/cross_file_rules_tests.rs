//! Cross-file LSP tests: a mock client drives the real server
//! loop over `Connection::memory()`, with a temp workspace on disk so the
//! cross-file pass can walk `.dart` files. Mirrors the harness in `lsp_tests.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeResult,
    PublishDiagnosticsParams, TextDocumentIdentifier, TextDocumentItem, Uri,
};
use tempfile::TempDir;

use falcon_lsp::{ServerOptions, run_with_connection};

const RECV_TIMEOUT: Duration = Duration::from_secs(5);
/// Quiet window used to confirm no further publish arrives.
const QUIET: Duration = Duration::from_millis(200);

/// Only `unused-files` is enabled as a cross-file rule (recommended preset off, so
/// `unused-code` stays quiet); the linter defaults remain on.
const ENABLE_UNUSED_FILES: &str = r#"{
    "cross-file": { "rules": { "recommended": false, "correctness": { "unused-files": "warn" } } }
}"#;
/// Master switch off: every cross-file rule resolves disabled, so the pass is skipped.
const DISABLE_CROSS_FILE: &str = r#"{ "cross-file": { "enabled": false } }"#;

struct TestClient {
    client: Connection,
    server: Option<JoinHandle<()>>,
    next_id: i32,
    workspace: TempDir,
}

impl TestClient {
    /// Start the server over a memory connection with `falcon.json` written to a
    /// fresh temp workspace; the workspace dir is the cross-file-pass walk root.
    fn start(config_json: &str) -> Self {
        let workspace = TempDir::new().unwrap();
        let config_path = workspace.path().join("falcon.json");
        fs::write(&config_path, config_json).unwrap();

        let (server_conn, client_conn) = Connection::memory();
        let options = ServerOptions {
            debounce: Duration::ZERO,
            config_path: Some(config_path),
        };
        let server = std::thread::spawn(move || {
            run_with_connection(server_conn, options).expect("server loop failed");
        });
        let mut this = Self {
            client: client_conn,
            server: Some(server),
            next_id: 0,
            workspace,
        };
        this.initialize();
        this
    }

    fn workspace_file(&self, name: &str) -> PathBuf {
        self.workspace.path().join(name)
    }

    /// Write a file at a (possibly nested) workspace-relative path, creating
    /// parent directories. Returns its `file://` URI.
    fn write_file(&self, rel: &str, source: &str) -> String {
        let path = self.workspace_file(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, source).unwrap();
        uri_for(&path)
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

    fn recv_response(&self, id: &RequestId) -> lsp_server::Response {
        match self.recv() {
            Message::Response(resp) => {
                assert_eq!(&resp.id, id, "response id must echo the request id");
                resp
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

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

    /// Drain every publish currently queued (within a short quiet window).
    fn drain_publishes(&self) -> Vec<PublishDiagnosticsParams> {
        let mut out = Vec::new();
        while let Ok(Message::Notification(note)) = self.client.receiver.recv_timeout(QUIET) {
            assert_eq!(
                note.method,
                lsp_types::notification::PublishDiagnostics::METHOD
            );
            out.push(serde_json::from_value(note.params).expect("PublishDiagnosticsParams shape"));
        }
        out
    }

    fn initialize(&mut self) {
        let id = self.request(
            lsp_types::request::Initialize::METHOD,
            lsp_types::InitializeParams::default(),
        );
        let resp = self.recv_response(&id);
        let _: InitializeResult =
            serde_json::from_value(resp.result.expect("initialize result")).expect("shape");
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

    fn save(&self, uri: &str) {
        self.notify(
            lsp_types::notification::DidSaveTextDocument::METHOD,
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: Uri::from_str(uri).unwrap(),
                },
                text: None,
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
            .expect("clean server exit");
    }
}

fn uri_for(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn has_rule(params: &PublishDiagnosticsParams, rule: &str) -> bool {
    params
        .diagnostics
        .iter()
        .any(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(c)) if c == rule))
}

/// Whether any drained publish for `uri` reports `rule`.
fn any_publish_has_rule(publishes: &[PublishDiagnosticsParams], uri: &str, rule: &str) -> bool {
    publishes
        .iter()
        .any(|p| p.uri.as_str() == uri && has_rule(p, rule))
}

/// Lay down a minimal referenced graph plus one dead file under `lib/` (the
/// rule's scope). `orphan.dart` is referenced by nothing and has no `main`, so
/// `unused-files` flags it.
fn write_corpus(client: &TestClient) -> String {
    client.write_file(
        "lib/main.dart",
        "import 'used.dart';\n\nvoid main() {\n  helper();\n}\n",
    );
    client.write_file("lib/used.dart", "void helper() {\n  print('helper');\n}\n");
    client.write_file("lib/orphan.dart", "class OrphanThing {}\n")
}

/// (1) didOpen a file that a cross-file rule flags → the published diagnostics for
/// that file include the cross-file rule.
#[test]
fn did_open_publishes_cross_file_diagnostic() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    let orphan_uri = write_corpus(&client);

    client.open(&orphan_uri, "class OrphanThing {}\n", 1);
    let publishes = client.drain_publishes();
    assert!(
        any_publish_has_rule(&publishes, &orphan_uri, "unused-files"),
        "orphan file must be flagged by unused-files: {publishes:?}"
    );
    client.shutdown();
}

/// (2) A file with no per-file issues, flagged only by a cross-file rule, still
/// gets the cross-file diagnostic. The first (per-file) publish is empty; a later
/// publish carries `unused-files` — proving the cross-file pass, not a file rule.
#[test]
fn clean_file_still_gets_cross_file_diagnostic() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    let orphan_uri = write_corpus(&client);

    client.open(&orphan_uri, "class OrphanThing {}\n", 1);
    let per_file = client.recv_publish();
    assert_eq!(per_file.uri.as_str(), orphan_uri);
    assert!(
        per_file.diagnostics.is_empty(),
        "clean file must have no per-file diagnostics: {per_file:?}"
    );
    let cross_file = client.recv_publish();
    assert!(
        has_rule(&cross_file, "unused-files"),
        "cross-file pass must add unused-files: {cross_file:?}"
    );

    // didSave also re-runs the cross-file pass and republishes the diagnostic.
    client.save(&orphan_uri);
    let after_save = client.drain_publishes();
    assert!(
        any_publish_has_rule(&after_save, &orphan_uri, "unused-files"),
        "didSave must republish the cross-file diagnostic: {after_save:?}"
    );
    client.shutdown();
}

/// Conditional (`if (dart.library.io) '...'`) import/export targets are real
/// reference edges: the platform-specific `lib/src` impl files are reachable
/// from the public-surface `lib/api.dart` and must NOT be flagged. An
/// unreferenced `lib/src/orphan.dart` is the positive control.
#[test]
fn conditional_directive_targets_not_flagged_unused() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    client.write_file("pubspec.yaml", "name: cond\n");
    let api_src = "export 'src/impl_stub.dart'\n    if (dart.library.io) 'src/impl_io.dart'\n    if (dart.library.html) 'src/impl_web.dart';\n";
    let api = client.write_file("lib/api.dart", api_src);
    client.write_file("lib/src/impl_stub.dart", "class Impl {}\n");
    let io = client.write_file("lib/src/impl_io.dart", "class Impl {}\n");
    let web = client.write_file("lib/src/impl_web.dart", "class Impl {}\n");
    let orphan_src = "class Orphan {}\n";
    let orphan = client.write_file("lib/src/orphan.dart", orphan_src);

    client.open(&api, api_src, 1);
    client.open(&io, "class Impl {}\n", 1);
    client.open(&web, "class Impl {}\n", 1);
    client.open(&orphan, orphan_src, 1);
    let pubs = client.drain_publishes();

    assert!(
        any_publish_has_rule(&pubs, &orphan, "unused-files"),
        "control: unreachable lib/src orphan must be flagged: {pubs:?}"
    );
    assert!(
        !any_publish_has_rule(&pubs, &api, "unused-files"),
        "public-surface lib/api.dart is an entrypoint external consumers import: {pubs:?}"
    );
    assert!(
        !any_publish_has_rule(&pubs, &io, "unused-files"),
        "conditional `if (dart.library.io)` target must not be flagged: {pubs:?}"
    );
    assert!(
        !any_publish_has_rule(&pubs, &web, "unused-files"),
        "conditional `if (dart.library.html)` target must not be flagged: {pubs:?}"
    );
    client.shutdown();
}

/// The upgrade to reachability: a `lib/src` island (two files referencing only
/// each other, unreachable from any entrypoint) is dead — both are flagged, even
/// though each is "referenced" by the other.
#[test]
fn dead_code_island_both_flagged() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    client.write_file("pubspec.yaml", "name: island\n");
    // Live surface, reachable, as a control that the rule isn't flagging blindly.
    client.write_file("lib/island.dart", "export 'src/live.dart';\n");
    client.write_file("lib/src/live.dart", "class Live {}\n");
    // Dead island: a <-> b reference each other and nothing else reaches them.
    let a_src = "import 'b.dart';\nclass A { B? b; }\n";
    let b_src = "import 'a.dart';\nclass B { A? a; }\n";
    let a = client.write_file("lib/src/a.dart", a_src);
    let b = client.write_file("lib/src/b.dart", b_src);

    client.open(&a, a_src, 1);
    client.open(&b, b_src, 1);
    let pubs = client.drain_publishes();

    assert!(
        any_publish_has_rule(&pubs, &a, "unused-files"),
        "island file a must be flagged: {pubs:?}"
    );
    assert!(
        any_publish_has_rule(&pubs, &b, "unused-files"),
        "island file b must be flagged: {pubs:?}"
    );
    client.shutdown();
}

/// The package barrel `lib/<pkg>.dart` is the public entrypoint external
/// consumers import, so it is never referenced from within its own package and
/// must not be flagged. An unreferenced non-barrel lib file is the control.
#[test]
fn package_barrel_not_flagged_unused() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    client.write_file("pubspec.yaml", "name: mypkg\n");
    let barrel_src = "export 'src/foo.dart';\nexport 'src/bar.dart';\n";
    let barrel = client.write_file("lib/mypkg.dart", barrel_src);
    client.write_file("lib/src/foo.dart", "class Foo {}\n");
    client.write_file("lib/src/bar.dart", "class Bar {}\n");
    let dead_src = "class Dead {}\n";
    let dead = client.write_file("lib/src/dead.dart", dead_src);

    client.open(&barrel, barrel_src, 1);
    client.open(&dead, dead_src, 1);
    let pubs = client.drain_publishes();

    assert!(
        !any_publish_has_rule(&pubs, &barrel, "unused-files"),
        "package barrel lib/mypkg.dart is the public entrypoint: {pubs:?}"
    );
    assert!(
        any_publish_has_rule(&pubs, &dead, "unused-files"),
        "control: unreachable lib/src file must still be flagged: {pubs:?}"
    );
    client.shutdown();
}

/// Without a pubspec the rule's documented scope (files under `lib/`) still
/// holds: a `test/` helper must not be flagged. A `lib/` orphan is the control.
#[test]
fn pubspec_less_non_lib_files_not_flagged() {
    let mut client = TestClient::start(ENABLE_UNUSED_FILES);
    let helper_src = "void helper() {}\n";
    let helpers = client.write_file("test/helpers.dart", helper_src);
    let orphan_src = "class Orphan {}\n";
    let orphan = client.write_file("lib/orphan.dart", orphan_src);

    client.open(&helpers, helper_src, 1);
    client.open(&orphan, orphan_src, 1);
    let pubs = client.drain_publishes();

    assert!(
        !any_publish_has_rule(&pubs, &helpers, "unused-files"),
        "pubspec-less test/ helper is out of the rule's lib/ scope: {pubs:?}"
    );
    assert!(
        any_publish_has_rule(&pubs, &orphan, "unused-files"),
        "control: pubspec-less lib/ orphan must still be flagged: {pubs:?}"
    );
    client.shutdown();
}

/// (3) With cross-file rules disabled, no cross-file diagnostics are published — only
/// the single per-file publish for the opened document.
#[test]
fn disabled_cross_file_rules_publish_nothing() {
    let mut client = TestClient::start(DISABLE_CROSS_FILE);
    let orphan_uri = write_corpus(&client);

    client.open(&orphan_uri, "class OrphanThing {}\n", 1);
    let publishes = client.drain_publishes();
    assert_eq!(
        publishes.len(),
        1,
        "disabled cross-file rules must yield exactly one (per-file) publish: {publishes:?}"
    );
    assert!(
        !has_rule(&publishes[0], "unused-files"),
        "no cross-file diagnostic when disabled: {publishes:?}"
    );
    client.shutdown();
}
