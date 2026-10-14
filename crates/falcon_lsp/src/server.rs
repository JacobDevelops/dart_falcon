//! LSP server loop: single-threaded message dispatch over `lsp-server`.
//!
//! Transport-agnostic: `run_server` binds stdio for production; tests drive
//! the identical loop through `run_with_connection` + `Connection::memory()`.

use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use lsp_server::{
    Connection, ErrorCode, Message, Notification as ServerNotification, Request, Response,
};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{HoverRequest, Request as _};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeResult, MarkupContent, MarkupKind, PositionEncodingKind, PublishDiagnosticsParams,
    SaveOptions, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, Uri,
};
use tracing::{debug, info, warn};

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
    // Initialize handshake (JSON-RPC request id is echoed by initialize_finish).
    let (initialize_id, _initialize_params) = connection.initialize_start()?;
    let initialize_result = InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(ServerInfo {
            name: "falcon".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    connection.initialize_finish(initialize_id, serde_json::to_value(initialize_result)?)?;
    info!("LSP server initialized");

    let mut state = LspState::new(options.config_path.clone());
    let mut debounce = Debounce::default();

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
                    let uri = params.text_document.uri.as_str().to_string();
                    let diagnostics = state.save(&uri, params.text);
                    publish_affected_with_current(&connection, &mut state, &uri, &diagnostics)?;
                    debounce.clear(&uri);
                    cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                }
                DidCloseTextDocument::METHOD => {
                    let params: DidCloseTextDocumentParams = parse_params!(&method, params);
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
                    debug!("watched files changed — reloading config");
                    for (uri, diagnostics) in state.reload_config() {
                        debounce.clear(&uri);
                        publish(&connection, &state, &uri, &diagnostics)?;
                    }
                    cross_file_pass_and_publish(&connection, &mut state, &mut debounce)?;
                }
                // `handle_shutdown` consumes the `exit` that follows a `shutdown`
                // request, so reaching this arm means `exit` arrived with no prior
                // `shutdown` — the spec requires the process to exit with code 1.
                "exit" => return Err("exit notification received without prior shutdown".into()),
                other => debug!(method = other, "ignoring notification"),
            },
            Message::Response(_) => {}
        }
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
        &document.text,
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
                .map(|d| d.format_lsp(&document.text))
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
