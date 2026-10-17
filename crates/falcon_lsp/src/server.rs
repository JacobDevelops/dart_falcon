//! LSP server loop: single-threaded message dispatch over `lsp-server`.
//!
//! Transport-agnostic: `run_server` binds stdio for production; tests drive
//! the identical loop through `run_with_connection` + `Connection::memory()`.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use lsp_server::{
    Connection, ErrorCode, Message, Notification as ServerNotification, Request, RequestId,
    Response,
};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{HoverRequest, RegisterCapability, Request as _, UnregisterCapability};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileSystemWatcher, GlobPattern, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeResult, MarkupContent,
    MarkupKind, OneOf, PositionEncodingKind, PublishDiagnosticsParams, Registration,
    RegistrationParams, RelativePattern, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Unregistration, UnregistrationParams, Uri,
};
use tracing::{debug, info, warn};

use falcon_analyze::canonical_or_lexical;
use falcon_diagnostics::{Diagnostic, lsp_position_to_byte};

use crate::state::LspState;

/// Server configuration. `debounce` is the quiet period after the last
/// `didChange` before re-analysis; tests use `Duration::ZERO`.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub debounce: Duration,
    /// Explicit falcon.json path; `None` uses the standard discovery order.
    pub config_path: Option<PathBuf>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(500),
            config_path: None,
        }
    }
}

type ServerError = Box<dyn Error + Send + Sync>;

#[derive(Default)]
struct Debounce {
    dirty: HashSet<String>,
    deadline: Option<Instant>,
}

impl Debounce {
    fn clear(&mut self, uri: &str) {
        self.dirty.remove(uri);
        if self.dirty.is_empty() {
            self.deadline = None;
        }
    }

    fn schedule(&mut self, uri: String, delay: Duration) {
        self.dirty.insert(uri);
        self.deadline = Some(Instant::now() + delay);
    }

    fn drain(&mut self) -> Vec<String> {
        self.deadline = None;
        let mut uris = self.dirty.drain().collect::<Vec<_>>();
        uris.sort();
        uris
    }
}

/// Client-owned file watching: configuration globs are workspace-relative;
/// mutable dependencies outside the workspace require relative-pattern support.
struct FileWatching {
    dynamic: bool,
    relative: bool,
    workspace_roots: Vec<PathBuf>,
    dependency_roots: Vec<PathBuf>,
    dependency_files: Vec<PathBuf>,
    registration: Option<String>,
    generation: u64,
    pending: HashMap<RequestId, &'static str>,
    dependency_generation: Option<u64>,
}

impl FileWatching {
    fn new(params: &serde_json::Value) -> Self {
        let dynamic = params
            .pointer("/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        let relative = params
            .pointer("/capabilities/workspace/didChangeWatchedFiles/relativePatternSupport")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        let workspace_roots = params
            .get("workspaceFolders")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|folder| folder.get("uri").and_then(serde_json::Value::as_str))
            .chain(params.get("rootUri").and_then(serde_json::Value::as_str))
            .filter_map(|text| Uri::from_str(text).ok())
            .filter(|uri| uri.scheme().is_some_and(|scheme| scheme.as_str() == "file"))
            .map(|uri| canonical_or_lexical(&crate::state::uri_to_path(uri.as_str())))
            .collect();
        Self {
            dynamic,
            relative,
            workspace_roots,
            dependency_roots: Vec::new(),
            dependency_files: Vec::new(),
            registration: None,
            generation: 0,
            pending: HashMap::new(),
            dependency_generation: None,
        }
    }

    fn refresh(&mut self, connection: &Connection, state: &LspState) -> Result<(), ServerError> {
        if !self.dynamic {
            return Ok(());
        }
        let dependency_generation = state.watched_dependency_generation();
        if self.registration.is_some()
            && (!self.relative || self.dependency_generation == Some(dependency_generation))
        {
            return Ok(());
        }
        self.dependency_generation = Some(dependency_generation);
        let mut roots: Vec<PathBuf> = if self.relative {
            state
                .watched_dependency_roots()
                .into_iter()
                .filter(|root| {
                    !self
                        .workspace_roots
                        .iter()
                        .any(|workspace| root.starts_with(workspace))
                })
                .collect()
        } else {
            Vec::new()
        };
        roots.sort_unstable();
        roots.dedup();
        let mut files: Vec<PathBuf> = if self.relative {
            state
                .watched_dependency_files()
                .iter()
                .filter(|file| {
                    !self
                        .workspace_roots
                        .iter()
                        .any(|workspace| file.starts_with(workspace))
                        && !roots.iter().any(|root| file.starts_with(root))
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        files.sort_unstable();
        files.dedup();
        if self.registration.is_some()
            && roots == self.dependency_roots
            && files == self.dependency_files
        {
            return Ok(());
        }
        self.generation += 1;
        if let Some(id) = self.registration.take() {
            let request = Request::new(
                format!("falcon-watch-unregister-{}", self.generation).into(),
                UnregisterCapability::METHOD.to_string(),
                UnregistrationParams {
                    unregisterations: vec![Unregistration {
                        id,
                        method: DidChangeWatchedFiles::METHOD.to_string(),
                    }],
                },
            );
            self.pending
                .insert(request.id.clone(), UnregisterCapability::METHOD);
            connection.sender.send(request.into())?;
        }
        let mut watchers = Vec::with_capacity(6 + roots.len() * 2 + files.len());
        watchers.extend(
            [
                "**/falcon.json",
                "**/.falcon.json",
                "**/.dart_tool/package_config.json",
                "**/pubspec.{yaml,lock}",
                "**/pubspec_overrides.yaml",
                "**/*.dart",
            ]
            .into_iter()
            .map(|pattern| FileSystemWatcher {
                glob_pattern: GlobPattern::String(pattern.to_string()),
                kind: None,
            }),
        );
        for root in &roots {
            watchers.push(relative_watcher(root, "**/*".to_string())?);
            // Watching the parent entry also observes deletion/recreation of the
            // dependency directory itself, which its descendant glob cannot.
            if let Some(parent) = root.parent() {
                watchers.push(relative_watcher(parent, "*".to_string())?);
            }
        }
        for file in &files {
            if let (Some(parent), Some(name)) = (file.parent(), file.file_name()) {
                let name = name.to_string_lossy();
                let mut pattern = String::with_capacity(name.len() * 3);
                for character in name.chars() {
                    match character {
                        '*' | '?' | '[' | ']' | '{' | '}' => {
                            pattern.push('[');
                            pattern.push(character);
                            pattern.push(']');
                        }
                        _ => pattern.push(character),
                    }
                }
                watchers.push(relative_watcher(parent, pattern)?);
            }
        }
        let id = format!("falcon-watched-files-{}", self.generation);
        let request = Request::new(
            format!("falcon-watch-register-{}", self.generation).into(),
            RegisterCapability::METHOD.to_string(),
            RegistrationParams {
                registrations: vec![Registration {
                    id: id.clone(),
                    method: DidChangeWatchedFiles::METHOD.to_string(),
                    register_options: Some(serde_json::to_value(
                        lsp_types::DidChangeWatchedFilesRegistrationOptions { watchers },
                    )?),
                }],
            },
        );
        self.pending
            .insert(request.id.clone(), RegisterCapability::METHOD);
        connection.sender.send(request.into())?;
        self.registration = Some(id);
        self.dependency_roots = roots;
        self.dependency_files = files;
        Ok(())
    }

    fn response(&mut self, response: Response) {
        if let Some(method) = self.pending.remove(&response.id)
            && let Some(error) = response.error
        {
            warn!(method, code = error.code, message = %error.message,
                "client rejected file watching; using configuration-save fallback");
            self.dynamic = false;
            self.registration = None;
            self.dependency_roots.clear();
        }
    }
}

fn file_uri(path: &Path) -> Result<Uri, ServerError> {
    let mut uri = String::with_capacity(8 + path.as_os_str().len() * 3);
    uri.push_str("file://");
    if !path.as_os_str().as_encoded_bytes().starts_with(b"/") {
        uri.push('/');
    }
    for &byte in path.as_os_str().as_encoded_bytes() {
        if byte == b'\\' && std::path::MAIN_SEPARATOR == '\\' {
            uri.push('/');
        } else if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            write!(uri, "%{byte:02X}")?;
        }
    }
    Ok(Uri::from_str(&uri)?)
}

fn relative_watcher(base: &Path, pattern: String) -> Result<FileSystemWatcher, ServerError> {
    Ok(FileSystemWatcher {
        glob_pattern: GlobPattern::Relative(RelativePattern {
            base_uri: OneOf::Right(file_uri(base)?),
            pattern,
        }),
        kind: None,
    })
}

fn is_environment_document(uri: &Uri) -> bool {
    let path = uri.path().as_estr().decode().into_string_lossy();
    matches!(
        path.rsplit('/').next(),
        Some(
            "falcon.json"
                | ".falcon.json"
                | "package_config.json"
                | "pubspec.yaml"
                | "pubspec.lock"
                | "pubspec_overrides.yaml"
        )
    )
}

fn watched_files_and_publish(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
    paths: &[PathBuf],
) -> Result<(), ServerError> {
    if paths.is_empty() {
        return Ok(());
    }
    let affected = state.watched_files_changed(paths);
    if affected.is_empty() {
        return Ok(());
    }
    for (uri, diagnostics) in affected {
        debounce.clear(&uri);
        publish(connection, state, &uri, &diagnostics)?;
    }
    cross_file_pass_and_publish(connection, state, debounce)
}

/// Capabilities advertised in the initialize response.
fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // Positions use UTF-16 code units (the LSP default); advertise it so
        // clients don't silently assume a different encoding.
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                will_save: None,
                will_save_wait_until: None,
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
            },
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..Default::default()
    }
}

/// Start the LSP server on stdio. Blocks until the client disconnects or
/// completes the shutdown/exit sequence.
pub fn run_server() -> Result<(), ServerError> {
    let (connection, io_threads) = Connection::stdio();
    run_with_connection(connection, ServerOptions::default())?;
    io_threads.join()?;
    info!("LSP server stopped");
    Ok(())
}

/// Run the server loop over an arbitrary connection (stdio in production,
/// `Connection::memory()` in tests).
pub fn run_with_connection(
    connection: Connection,
    options: ServerOptions,
) -> Result<(), ServerError> {
    let state = LspState::new(options.config_path.clone());
    run_with_state(connection, options, state)
}

fn run_with_state(
    connection: Connection,
    options: ServerOptions,
    mut state: LspState,
) -> Result<(), ServerError> {
    // Initialize handshake (JSON-RPC request id is echoed by initialize_finish).
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let mut watching = FileWatching::new(&initialize_params);
    let initialize_result = InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(ServerInfo {
            name: "falcon".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    connection.initialize_finish(initialize_id, serde_json::to_value(initialize_result)?)?;
    info!("LSP server initialized");

    let mut debounce = Debounce::default();
    watching.refresh(&connection, &state)?;

    // Deserialize a notification's params or skip the message: a single
    // malformed notification must not tear down the session (LSP spec).
    macro_rules! parse_params {
        ($method:expr, $params:expr) => {
            match serde_json::from_value($params) {
                Ok(value) => value,
                Err(error) => {
                    warn!(method = %$method, %error, "ignoring notification with malformed params");
                    continue;
                }
            }
        };
    }

    loop {
        let message = match debounce.deadline {
            Some(at) => {
                let timeout = at.saturating_duration_since(Instant::now());
                match connection.receiver.recv_timeout(timeout) {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            }
            None => match connection.receiver.recv() {
                Ok(message) => Some(message),
                Err(_) => return Ok(()),
            },
        };

        let Some(message) = message else {
            // Debounce window elapsed: flush every dirty document.
            let uris = debounce.drain();
            for (affected_uri, diagnostics) in state.analyze_affected_many(&uris) {
                publish(&connection, &state, &affected_uri, &diagnostics)?;
            }
            cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
            continue;
        };

        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                handle_request(&connection, &state, request)?;
            }
            Message::Notification(ServerNotification { method, params }) => match method.as_str() {
                DidOpenTextDocument::METHOD => {
                    let params: DidOpenTextDocumentParams = parse_params!(&method, params);
                    if is_environment_document(&params.text_document.uri) {
                        continue;
                    }
                    let uri = params.text_document.uri.as_str().to_string();
                    let diagnostics = state.open(
                        &uri,
                        params.text_document.text,
                        Some(params.text_document.version),
                    );
                    debounce.clear(&uri);
                    publish_affected_with_current(&connection, &mut state, &uri, &diagnostics)?;
                    cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                }
                DidChangeTextDocument::METHOD => {
                    let params: DidChangeTextDocumentParams = parse_params!(&method, params);
                    if is_environment_document(&params.text_document.uri) {
                        continue;
                    }
                    let uri = params.text_document.uri.as_str().to_string();
                    // FULL sync: the last change event carries the whole document.
                    let Some(change) = params.content_changes.into_iter().next_back() else {
                        continue;
                    };
                    if !state.change(&uri, change.text, Some(params.text_document.version)) {
                        continue;
                    }
                    if options.debounce.is_zero() {
                        analyze_affected_and_publish(&connection, &mut state, &uri)?;
                        cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                    } else {
                        debounce.schedule(uri, options.debounce);
                    }
                }
                DidSaveTextDocument::METHOD => {
                    let params: DidSaveTextDocumentParams = parse_params!(&method, params);
                    if is_environment_document(&params.text_document.uri) {
                        if !watching.dynamic {
                            watched_files_and_publish(
                                &connection,
                                &mut state,
                                &mut debounce,
                                &[crate::state::uri_to_path(params.text_document.uri.as_str())],
                            )?;
                            watching.refresh(&connection, &state)?;
                        }
                        continue;
                    }
                    let uri = params.text_document.uri.as_str().to_string();
                    let diagnostics = state.save(&uri, params.text);
                    publish_affected_with_current(&connection, &mut state, &uri, &diagnostics)?;
                    debounce.clear(&uri);
                    cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                }
                DidCloseTextDocument::METHOD => {
                    let params: DidCloseTextDocumentParams = parse_params!(&method, params);
                    if is_environment_document(&params.text_document.uri) {
                        continue;
                    }
                    let uri = params.text_document.uri.as_str().to_string();
                    let mut affected = state.affected_open_uris(&uri);
                    state.close(&uri);
                    affected.extend(state.affected_open_uris(&uri));
                    affected.sort();
                    affected.dedup();
                    debounce.clear(&uri);
                    // Clear stale squiggles before refreshing dependents against
                    // the closed document's on-disk workspace state.
                    publish(&connection, &state, &uri, &[])?;
                    for affected_uri in affected {
                        if affected_uri == uri {
                            continue;
                        }
                        let diagnostics = state.analyze(&affected_uri);
                        publish(&connection, &state, &affected_uri, &diagnostics)?;
                    }
                    cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                }
                DidChangeWatchedFiles::METHOD => {
                    let params: DidChangeWatchedFilesParams = parse_params!(&method, params);
                    let paths: Vec<_> = params
                        .changes
                        .into_iter()
                        .filter(|event| {
                            event
                                .uri
                                .scheme()
                                .is_some_and(|scheme| scheme.as_str() == "file")
                        })
                        .map(|event| crate::state::uri_to_path(event.uri.as_str()))
                        .collect();
                    watched_files_and_publish(&connection, &mut state, &mut debounce, &paths)?;
                }
                // `handle_shutdown` consumes the `exit` that follows a `shutdown`
                // request, so reaching this arm means `exit` arrived with no prior
                // `shutdown` — the spec requires the process to exit with code 1.
                "exit" => return Err("exit notification received without prior shutdown".into()),
                other => debug!(method = other, "ignoring notification"),
            },
            Message::Response(response) => watching.response(response),
        }
        watching.refresh(&connection, &state)?;
    }
}

fn handle_request(
    connection: &Connection,
    state: &LspState,
    request: Request,
) -> Result<(), ServerError> {
    match request.method.as_str() {
        HoverRequest::METHOD => {
            let params: HoverParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                // Malformed params get an InvalidParams error, not a dead server.
                Err(error) => {
                    connection.sender.send(
                        Response::new_err(
                            request.id,
                            ErrorCode::InvalidParams as i32,
                            format!("invalid params: {error}"),
                        )
                        .into(),
                    )?;
                    return Ok(());
                }
            };
            let result = hover(state, &params);
            connection
                .sender
                .send(Response::new_ok(request.id, result).into())?;
        }
        other => {
            warn!(method = other, "unsupported request");
            connection.sender.send(
                Response::new_err(
                    request.id,
                    ErrorCode::MethodNotFound as i32,
                    format!("method not supported: {other}"),
                )
                .into(),
            )?;
        }
    }
    Ok(())
}

/// Hover: surface the falcon diagnostics under the cursor (rule id + message).
fn hover(state: &LspState, params: &HoverParams) -> Option<Hover> {
    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .as_str();
    let document = state.document(uri)?;
    let offset = lsp_position_to_byte(
        document.text(),
        params.text_document_position_params.position,
    );
    let lines: Vec<String> = document
        .last_diagnostics
        .iter()
        .filter(|d| d.span.start <= offset && offset < d.span.end.max(d.span.start + 1))
        .map(|d| format!("**{}**: {}", d.rule, d.message))
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n\n"),
        }),
        range: None,
    })
}

/// Run the cross-file pass and publish merged diagnostics for every open
/// document whose cross-file set changed. A no-op when no cross-file rule is
/// enabled, so per-file-only configs keep their exact prior publish behavior.
fn cross_file_pass_and_publish(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
) -> Result<(), ServerError> {
    if !state.cross_file_rules_enabled() {
        return Ok(());
    }
    let uris = debounce.drain();
    for (uri, diagnostics) in state.analyze_affected_many(&uris) {
        publish(connection, state, &uri, &diagnostics)?;
    }
    for (uri, diagnostics) in state.cross_file_pass() {
        publish(connection, state, &uri, &diagnostics)?;
    }
    Ok(())
}

/// Publish the already-computed result for `uri`, then re-analyze and publish
/// every other open semantic dependent.
fn publish_affected_with_current(
    connection: &Connection,
    state: &mut LspState,
    uri: &str,
    diagnostics: &[Diagnostic],
) -> Result<(), ServerError> {
    for affected_uri in state.affected_open_uris(uri) {
        if affected_uri == uri {
            publish(connection, state, &affected_uri, diagnostics)?;
        } else {
            let diagnostics = state.analyze(&affected_uri);
            publish(connection, state, &affected_uri, &diagnostics)?;
        }
    }
    Ok(())
}

/// Analyze `uri` and any open semantic dependents, then publish each result.
fn analyze_affected_and_publish(
    connection: &Connection,
    state: &mut LspState,
    uri: &str,
) -> Result<(), ServerError> {
    for (affected_uri, diagnostics) in state.analyze_affected_many(&[uri.to_string()]) {
        publish(connection, state, &affected_uri, &diagnostics)?;
    }
    Ok(())
}

/// Send `textDocument/publishDiagnostics` for `uri`.
fn publish(
    connection: &Connection,
    state: &LspState,
    uri: &str,
    diagnostics: &[Diagnostic],
) -> Result<(), ServerError> {
    let (lsp_diagnostics, version) = match state.document(uri) {
        Some(document) => (
            diagnostics
                .iter()
                .map(|d| d.format_lsp(document.text()))
                .collect(),
            document.version,
        ),
        // Closed document: publish an empty set to clear editor state.
        None => (Vec::new(), None),
    };
    let params = PublishDiagnosticsParams {
        uri: Uri::from_str(uri).map_err(|e| format!("invalid uri {uri}: {e}"))?,
        diagnostics: lsp_diagnostics,
        version,
    };
    connection
        .sender
        .send(ServerNotification::new(PublishDiagnostics::METHOD.to_string(), params).into())?;
    Ok(())
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
            let directory = tempfile::tempdir().unwrap();
            let workspace = directory.path().join("workspace");
            std::fs::create_dir_all(workspace.join(".dart_tool")).unwrap();
            std::fs::write(workspace.join("falcon.json"), CONFIG).unwrap();
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
                .all(|watcher| matches!(watcher.glob_pattern, GlobPattern::String(_)))
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
