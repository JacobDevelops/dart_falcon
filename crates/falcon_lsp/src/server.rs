//! Responsive LSP transport with a dedicated, serial analysis-state worker.
//!
//! Transport-agnostic: `run_server` binds stdio for production; tests drive
//! the identical loop through `run_with_connection` + `Connection::memory()`.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use lsp_server::{
    Connection, ErrorCode, Message, Notification as ServerNotification, Request, RequestId,
    Response,
};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Notification as _, PublishDiagnostics, ShowMessage,
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

type WorkerMessage = (u64, Message, Option<Arc<AtomicBool>>);

#[derive(Default)]
struct Debounce {
    dirty: Arc<HashSet<String>>,
    deadline: Option<Instant>,
}

impl Debounce {
    fn clear(&mut self, uri: &str) {
        if self.dirty.contains(uri) {
            Arc::make_mut(&mut self.dirty).remove(uri);
        }
        if self.dirty.is_empty() {
            self.deadline = None;
        }
    }

    fn schedule(&mut self, uri: String, delay: Duration) {
        if !self.dirty.contains(&uri) {
            Arc::make_mut(&mut self.dirty).insert(uri);
        }
        self.deadline = Some(Instant::now() + delay);
    }

    fn drain(&mut self) -> Vec<String> {
        self.deadline = None;
        if self.dirty.is_empty() {
            return Vec::new();
        }
        let mut uris = if let Some(dirty) = Arc::get_mut(&mut self.dirty) {
            dirty.drain().collect::<Vec<_>>()
        } else {
            // In-flight rounds retain their immutable snapshot. Only the owned
            // URI list needs copying, not another hash table and its capacity.
            std::mem::take(&mut self.dirty).iter().cloned().collect()
        };
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
        let folders = params
            .get("workspaceFolders")
            .and_then(serde_json::Value::as_array)
            .filter(|folders| !folders.is_empty());
        let uris = folders
            .into_iter()
            .flatten()
            .filter_map(|folder| folder.get("uri").and_then(serde_json::Value::as_str))
            .chain(
                params
                    .get("rootUri")
                    .filter(|_| folders.is_none())
                    .and_then(serde_json::Value::as_str),
            );
        let mut workspace_roots = Vec::with_capacity(folders.map_or(1, Vec::len));
        for uri in uris.filter_map(|text| Uri::from_str(text).ok()) {
            if uri.scheme().is_some_and(|scheme| scheme.as_str() == "file") {
                let root = canonical_or_lexical(&crate::state::uri_to_path(uri.as_str()));
                if !workspace_roots.contains(&root) {
                    workspace_roots.push(root);
                }
            }
        }
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
        let (roots, files) = self.dependency_paths(state);
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
        let mut watchers = Vec::with_capacity(1 + roots.len() * 2 + files.len());
        watchers.extend(["**/*"].into_iter().map(|pattern| FileSystemWatcher {
            glob_pattern: GlobPattern::String(pattern.to_string()),
            kind: None,
        }));
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

    fn dependency_paths(&self, state: &LspState) -> (Vec<PathBuf>, Vec<PathBuf>) {
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
        if self.relative {
            files.extend(state.ancestor_manifests());
        }
        files.sort_unstable();
        files.dedup();
        (roots, files)
    }

    fn response(&mut self, response: Response) {
        if let Some(method) = self.pending.remove(&response.id)
            && let Some(error) = response.error
        {
            warn!(method, code = error.code, message = %error.message,
                "client rejected file watching; using quiet-boundary fallback");
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
                | "melos.yaml"
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
    publish_config_messages(connection, state)?;
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
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(
            lsp_types::CodeActionOptions {
                code_action_kinds: Some(vec![
                    lsp_types::CodeActionKind::QUICKFIX,
                    lsp_types::CodeActionKind::from(crate::state::code_actions::SAFE_FIX_ALL),
                    lsp_types::CodeActionKind::from(crate::state::code_actions::UNSAFE_FIX_ALL),
                ]),
                resolve_provider: Some(true),
                ..Default::default()
            },
        )),
        execute_command_provider: Some(lsp_types::ExecuteCommandOptions {
            commands: vec![crate::state::code_actions::FIX_ALL_COMMAND.to_owned()],
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Start the LSP server on stdio. Blocks until the client disconnects or
/// completes the shutdown/exit sequence.
///
/// # Errors
///
/// Returns an error if the connection, server loop, or I/O threads fail.
pub fn run_server() -> Result<(), ServerError> {
    let io_threads = {
        let (connection, io_threads) = Connection::stdio();
        run_with_connection(&connection, &ServerOptions::default())?;
        io_threads
    };
    io_threads.join()?;
    info!("LSP server stopped");
    Ok(())
}

/// Run the server loop over an arbitrary connection (stdio in production,
/// `Connection::memory()` in tests).
///
/// # Errors
///
/// Returns an error if initialization or the server loop fails.
pub fn run_with_connection(
    connection: &Connection,
    options: &ServerOptions,
) -> Result<(), ServerError> {
    let state = LspState::new(options.config_path.clone());
    run_with_state(connection, options, state)
}

fn run_with_state(
    connection: &Connection,
    options: &ServerOptions,
    mut state: LspState,
) -> Result<(), ServerError> {
    // Initialize handshake (JSON-RPC request id is echoed by initialize_finish).
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let watching = FileWatching::new(&initialize_params);
    state.initialize_workspace_roots(watching.workspace_roots.clone());
    state.configure_code_actions(&initialize_params);
    state.set_dynamic_file_watching(watching.dynamic);
    let initialize_result = InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(ServerInfo {
            name: "falcon".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    connection.initialize_finish(initialize_id, serde_json::to_value(initialize_result)?)?;
    info!("LSP server initialized");
    publish_config_messages(connection, &mut state)?;

    let (input, receiver) = crossbeam_channel::unbounded();
    let (sender, output) = crossbeam_channel::unbounded();
    let worker_options = options.clone();
    let worker = std::thread::Builder::new()
        .name("falcon-lsp-analysis".to_string())
        .spawn(move || {
            analysis_worker_with(&worker_options, state, watching, &receiver, &sender, |_| {})
        })?;
    let result = relay(connection, input, &output);
    let worker_result = worker.join().map_err(|_| "LSP analysis worker panicked")?;
    worker_result.and(result)
}

struct AnalysisRound {
    generation: u64,
    deferred: Arc<HashSet<String>>,
    messages: Vec<Message>,
}

/// The transport owns publication and cancellation; the worker alone owns state.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the relay must own this sender so it disconnects the worker before joining it"
)]
fn relay(
    connection: &Connection,
    input: crossbeam_channel::Sender<WorkerMessage>,
    output: &crossbeam_channel::Receiver<AnalysisRound>,
) -> Result<(), ServerError> {
    let mut generation = 0;
    let mut pending_requests = HashMap::<RequestId, Arc<AtomicBool>>::new();
    let mut publications = HashMap::new();
    let mut other_messages = Vec::new();
    let mut publication_order = 0_u64;
    let mut ready_publications = Vec::new();
    loop {
        crossbeam_channel::select_biased! {
            recv(connection.receiver) -> message => {
                let Ok(message) = message else { return Ok(()); };
                let mut cancelled = None;
                match &message {
                    // Only requests need a token; notifications always execute
                    // to preserve the worker's ordered document state.
                    Message::Request(request) => {
                        if request.method == "shutdown" {
                            // Resolve every accepted request before acknowledging
                            // shutdown; the worker's late responses are discarded.
                            for (id, token) in pending_requests.drain() {
                                token.store(true, Ordering::Relaxed);
                                connection.sender.send(Response::new_err(
                                    id,
                                    ErrorCode::RequestCanceled as i32,
                                    "server shutting down".to_string(),
                                ).into())?;
                            }
                            connection.handle_shutdown(request)?;
                            return Ok(());
                        }
                        let token = Arc::new(AtomicBool::new(false));
                        pending_requests.insert(request.id.clone(), Arc::clone(&token));
                        cancelled = Some(token);
                    }
                    Message::Notification(notification) if notification.method == "$/cancelRequest" => {
                        if let Some(id) = notification.params.get("id")
                            .and_then(|id| serde_json::from_value::<RequestId>(id.clone()).ok())
                            && let Some(token) = pending_requests.remove(&id)
                        {
                            token.store(true, Ordering::Relaxed);
                            connection.sender.send(Response::new_err(
                                id,
                                ErrorCode::RequestCanceled as i32,
                                "request cancelled".to_string(),
                            ).into())?;
                        }
                        continue;
                    }
                    Message::Notification(notification) => {
                        if notification.method == "exit" {
                            return Err("exit notification received without prior shutdown".into());
                        }
                        if matches!(notification.method.as_str(),
                            DidOpenTextDocument::METHOD | DidChangeTextDocument::METHOD
                            | DidSaveTextDocument::METHOD | DidCloseTextDocument::METHOD
                            | DidChangeWatchedFiles::METHOD)
                        {
                            generation += 1;
                        }
                    }
                    Message::Response(_) => {}
                }
                input.send((generation, message, cancelled))?;
            }
            recv(output) -> round => {
                let Ok(round) = round else { return Ok(()); };
                let current = round.generation == generation;
                // Keep unpublished unrelated files when a newer edit supersedes
                // a round. Its affected files replace these entries before flush.
                buffer_round_messages(
                    round.messages,
                    current,
                    &mut pending_requests,
                    &mut publications,
                    &mut other_messages,
                    &mut publication_order,
                )?;
                if current {
                    ready_publications.extend(publications.extract_if(|uri, _| !round.deferred.contains(uri)).map(|(_, publication)| publication));
                    ready_publications.sort_unstable_by_key(|(order, _)| *order);
                    for (_, notification) in ready_publications.drain(..) {
                        connection.sender.send(notification.into())?;
                    }
                }
                for message in other_messages.drain(..) {
                    connection.sender.send(message)?;
                }
            }
        }
    }
}

fn buffer_round_messages(
    messages: Vec<Message>,
    current: bool,
    pending_requests: &mut HashMap<RequestId, Arc<AtomicBool>>,
    publications: &mut HashMap<String, (u64, ServerNotification)>,
    other_messages: &mut Vec<Message>,
    publication_order: &mut u64,
) -> Result<(), ServerError> {
    other_messages.reserve(messages.len());
    for message in messages {
        match message {
            Message::Notification(notification)
                if notification.method == PublishDiagnostics::METHOD =>
            {
                if let Some(uri) = notification
                    .params
                    .get("uri")
                    .and_then(serde_json::Value::as_str)
                {
                    *publication_order += 1;
                    publications.insert(uri.to_string(), (*publication_order, notification));
                }
            }
            Message::Request(request) if request.method == "workspace/applyEdit" => {
                // Recover the originating executeCommand id without
                // colliding with other server-to-client request ids.
                let origin = serde_json::to_value(&request.id)?
                    .as_str()
                    .and_then(|id| id.strip_prefix("falcon-fix-all-"))
                    .and_then(|id| serde_json::from_str::<RequestId>(id).ok());
                // Never publish a side effect after cancellation or a
                // newer document notification overtook computation.
                if let Some(origin) = origin
                    && pending_requests.contains_key(&origin)
                {
                    if current {
                        other_messages.push(Message::Request(request));
                    } else {
                        pending_requests.remove(&origin);
                        other_messages.push(
                            Response::new_err(
                                origin,
                                ErrorCode::ContentModified as i32,
                                "Workspace changed while computing Falcon fixes".to_owned(),
                            )
                            .into(),
                        );
                    }
                }
            }
            Message::Response(response) => {
                if pending_requests.remove(&response.id).is_some() {
                    other_messages.push(Message::Response(response));
                }
            }
            message => other_messages.push(message),
        }
    }
    Ok(())
}

fn next_idle_poll_delay(current: Duration, changed: bool) -> Duration {
    if changed {
        Duration::from_secs(1)
    } else {
        (current * 2).min(Duration::from_secs(60))
    }
}

fn analysis_worker_with(
    options: &ServerOptions,
    mut state: LspState,
    mut watching: FileWatching,
    receiver: &crossbeam_channel::Receiver<WorkerMessage>,
    output: &crossbeam_channel::Sender<AnalysisRound>,
    mut before_message: impl FnMut(&Message),
) -> Result<(), ServerError> {
    // Buffer a complete round so preliminary per-file results never reach the
    // client before the cross-file pass has supplied the final merged set.
    let (sender, outbox) = crossbeam_channel::unbounded();
    let connection = Connection {
        sender,
        receiver: crossbeam_channel::never(),
    };
    let mut generation = 0;
    let mut debounce = Debounce::default();
    let mut idle_poll_delay = Duration::from_secs(1);
    watching.refresh(&connection, &state)?;

    loop {
        let messages: Vec<_> = outbox.try_iter().collect();
        // Even an ignored notification completes its generation. Otherwise an
        // unrelated superseding event could strand a valid buffered publication.
        if output
            .send(AnalysisRound {
                generation,
                deferred: Arc::clone(&debounce.dirty),
                messages,
            })
            .is_err()
        {
            return Ok(());
        }
        let message = match debounce.deadline {
            Some(at) => {
                let timeout = at.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(timeout) {
                    Ok((next_generation, message, cancelled)) => {
                        generation = next_generation;
                        Some((message, cancelled))
                    }
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            }
            None if watching.dynamic => match receiver.recv() {
                Ok((next_generation, message, cancelled)) => {
                    generation = next_generation;
                    Some((message, cancelled))
                }
                Err(_) => return Ok(()),
            },
            None => match receiver.recv_timeout(idle_poll_delay) {
                Ok((next_generation, message, cancelled)) => {
                    generation = next_generation;
                    Some((message, cancelled))
                }
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            },
        };

        let Some((message, token)) = message else {
            idle_poll_delay =
                revalidate_and_publish(&connection, &mut state, &mut debounce, idle_poll_delay)?;
            continue;
        };
        idle_poll_delay = Duration::from_secs(1);
        let cancelled = || {
            token
                .as_ref()
                .is_some_and(|token| token.load(Ordering::Relaxed))
        };
        if cancelled() {
            continue;
        }
        before_message(&message);

        match message {
            Message::Request(request) => {
                handle_request(&connection, &mut state, request, &cancelled)?;
            }
            Message::Notification(notification) => {
                if !handle_notification(
                    &connection,
                    &mut state,
                    &mut debounce,
                    &mut watching,
                    options.debounce,
                    notification,
                )? {
                    continue;
                }
            }
            Message::Response(response) => {
                watching.response(response);
                state.set_dynamic_file_watching(watching.dynamic);
            }
        }
        publish_config_messages(&connection, &mut state)?;
        watching.refresh(&connection, &state)?;
    }
}

fn revalidate_and_publish(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
    idle_poll_delay: Duration,
) -> Result<Duration, ServerError> {
    // Revalidate only after a quiet debounce/idle interval.
    let disk_updates = state.revalidate_disk();
    let refreshed = !disk_updates.is_empty();
    let idle_poll_delay = next_idle_poll_delay(idle_poll_delay, refreshed);
    for (uri, diagnostics) in disk_updates {
        publish(connection, state, &uri, &diagnostics)?;
    }
    let uris = debounce.drain();
    if uris.is_empty() && !refreshed {
        return Ok(idle_poll_delay);
    }
    for (affected_uri, diagnostics) in state.analyze_affected_many(&uris) {
        publish(connection, state, &affected_uri, &diagnostics)?;
    }
    cross_file_pass_and_publish(connection, state, debounce)?;
    publish_config_messages(connection, state)?;
    Ok(idle_poll_delay)
}

fn handle_notification(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
    watching: &mut FileWatching,
    delay: Duration,
    ServerNotification { method, params }: ServerNotification,
) -> Result<bool, ServerError> {
    // A malformed notification skips the message, not the session (LSP spec).
    macro_rules! parse_params {
        () => {
            match serde_json::from_value(params) {
                Ok(value) => value,
                Err(error) => {
                    warn!(method = %method, %error, "ignoring notification with malformed params");
                    return Ok(false);
                }
            }
        };
    }
    match method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = parse_params!();
            if is_environment_document(&params.text_document.uri) {
                return Ok(false);
            }
            open_and_publish(connection, state, debounce, params)?;
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = parse_params!();
            if is_environment_document(&params.text_document.uri) {
                return Ok(false);
            }
            let uri = params.text_document.uri.as_str().to_string();
            // FULL sync: the last change event carries the whole document.
            let Some(change) = params.content_changes.into_iter().next_back() else {
                return Ok(false);
            };
            if !state.change(&uri, change.text, Some(params.text_document.version)) {
                return Ok(false);
            }
            if delay.is_zero() {
                analyze_affected_and_publish(connection, state, &uri)?;
                cross_file_pass_and_publish(connection, state, debounce)?;
            } else {
                debounce.schedule(uri, delay);
            }
        }
        DidSaveTextDocument::METHOD => {
            let params: DidSaveTextDocumentParams = parse_params!();
            if is_environment_document(&params.text_document.uri) {
                if !watching.dynamic {
                    watched_files_and_publish(
                        connection,
                        state,
                        debounce,
                        &[crate::state::uri_to_path(params.text_document.uri.as_str())],
                    )?;
                    watching.refresh(connection, state)?;
                }
                return Ok(false);
            }
            let uri = params.text_document.uri.as_str().to_string();
            for (affected_uri, diagnostics) in state.revalidate_disk() {
                publish(connection, state, &affected_uri, &diagnostics)?;
            }
            let diagnostics = state.save(&uri, params.text);
            publish_affected_with_current(connection, state, &uri, &diagnostics)?;
            debounce.clear(&uri);
            cross_file_pass_and_publish(connection, state, debounce)?;
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = parse_params!();
            if is_environment_document(&params.text_document.uri) {
                return Ok(false);
            }
            close_and_publish(connection, state, debounce, &params)?;
        }
        DidChangeWatchedFiles::METHOD => {
            let params: DidChangeWatchedFilesParams = parse_params!();
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
            watched_files_and_publish(connection, state, debounce, &paths)?;
        }
        other => debug!(method = other, "ignoring notification"),
    }
    Ok(true)
}

fn open_and_publish(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
    params: DidOpenTextDocumentParams,
) -> Result<(), ServerError> {
    let uri = params.text_document.uri.as_str().to_string();
    for (affected_uri, diagnostics) in state.revalidate_disk() {
        publish(connection, state, &affected_uri, &diagnostics)?;
    }
    let diagnostics = state.open(
        &uri,
        params.text_document.text,
        Some(params.text_document.version),
    );
    debounce.clear(&uri);
    publish_affected_with_current(connection, state, &uri, &diagnostics)?;
    cross_file_pass_and_publish(connection, state, debounce)
}

fn close_and_publish(
    connection: &Connection,
    state: &mut LspState,
    debounce: &mut Debounce,
    params: &DidCloseTextDocumentParams,
) -> Result<(), ServerError> {
    let uri = params.text_document.uri.as_str().to_string();
    state.close(&uri);
    let affected = state.affected_open_uris(&uri);
    debounce.clear(&uri);
    // Clear stale squiggles before refreshing dependents against
    // the closed document's on-disk workspace state.
    publish(connection, state, &uri, &[])?;
    for affected_uri in affected {
        if affected_uri == uri {
            continue;
        }
        let diagnostics = state.analyze(&affected_uri);
        publish(connection, state, &affected_uri, &diagnostics)?;
    }
    cross_file_pass_and_publish(connection, state, debounce)
}

/// Deliver the state's deduplicated messages, batching warning lines into one popup.
fn publish_config_messages(
    connection: &Connection,
    state: &mut LspState,
) -> Result<(), ServerError> {
    let messages = state.take_config_messages();
    let (warning_count, warning_bytes) = messages
        .iter()
        .filter(|params| params.typ == lsp_types::MessageType::WARNING)
        .fold((0, 0), |(count, bytes), params| {
            (count + 1, bytes + params.message.len() + 1)
        });
    // Leave room for the count header as well as every warning line.
    let mut warning = String::with_capacity(if warning_count == 0 {
        0
    } else {
        warning_bytes + 64
    });
    if warning_count > 0 {
        write!(
            warning,
            "Falcon: {warning_count} configuration warning{}",
            if warning_count == 1 { "" } else { "s" }
        )?;
    }
    for params in messages {
        if params.typ == lsp_types::MessageType::WARNING {
            warning.push('\n');
            warning.push_str(&params.message);
        } else {
            connection
                .sender
                .send(ServerNotification::new(ShowMessage::METHOD.to_string(), params).into())?;
        }
    }
    if warning_count > 0 {
        let params = lsp_types::ShowMessageParams {
            typ: lsp_types::MessageType::WARNING,
            message: warning,
        };
        connection
            .sender
            .send(ServerNotification::new(ShowMessage::METHOD.to_string(), params).into())?;
    }
    Ok(())
}

fn handle_request(
    connection: &Connection,
    state: &mut LspState,
    request: Request,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), ServerError> {
    if cancelled() {
        return Ok(());
    }
    match request.method.as_str() {
        "textDocument/codeAction" => {
            let response =
                match serde_json::from_value::<lsp_types::CodeActionParams>(request.params) {
                    Ok(params) => match state.code_actions(&params, cancelled) {
                        Ok(actions) => Response::new_ok(request.id, actions),
                        Err(error) => {
                            Response::new_err(request.id, ErrorCode::RequestFailed as i32, error)
                        }
                    },
                    Err(error) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        format!("invalid params: {error}"),
                    ),
                };
            if !cancelled() {
                connection.sender.send(response.into())?;
            }
        }
        "codeAction/resolve" => {
            let response = match serde_json::from_value::<lsp_types::CodeAction>(request.params) {
                Ok(action) => match state.resolve_code_action(action, cancelled) {
                    Ok(action) => Response::new_ok(request.id, action),
                    Err(error) => {
                        Response::new_err(request.id, ErrorCode::RequestFailed as i32, error)
                    }
                },
                Err(error) => Response::new_err(
                    request.id,
                    ErrorCode::InvalidParams as i32,
                    format!("invalid params: {error}"),
                ),
            };
            if !cancelled() {
                connection.sender.send(response.into())?;
            }
        }
        "workspace/executeCommand" => {
            execute_command(connection, state, request, cancelled)?;
        }
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
            let result = hover(state, &params, cancelled);
            if !cancelled() {
                connection
                    .sender
                    .send(Response::new_ok(request.id, result).into())?;
            }
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

fn execute_command(
    connection: &Connection,
    state: &mut LspState,
    request: Request,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), ServerError> {
    let result = serde_json::from_value::<lsp_types::ExecuteCommandParams>(request.params);
    let response = match result {
        Ok(_) if !state.workspace_apply_edit => Response::new_err(
            request.id,
            ErrorCode::RequestFailed as i32,
            "Client does not support workspace/applyEdit".to_owned(),
        ),
        Ok(mut params)
            if params.command == crate::state::code_actions::FIX_ALL_COMMAND
                && params.arguments.len() == 1 =>
        {
            let action = lsp_types::CodeAction {
                title: "Fix all Falcon issues".to_owned(),
                data: params.arguments.pop(),
                ..Default::default()
            };
            match state.resolve_code_action(action, cancelled) {
                Ok(action) => {
                    if !cancelled()
                        && let Some(edit) = action.edit
                    {
                        connection.sender.send(
                            Request::new(
                                format!("falcon-fix-all-{}", serde_json::to_string(&request.id)?)
                                    .into(),
                                "workspace/applyEdit".to_owned(),
                                lsp_types::ApplyWorkspaceEditParams {
                                    label: Some(action.title),
                                    edit,
                                },
                            )
                            .into(),
                        )?;
                    }
                    Response::new_ok(request.id, serde_json::Value::Null)
                }
                Err(error) => Response::new_err(request.id, ErrorCode::RequestFailed as i32, error),
            }
        }
        Ok(_) => Response::new_err(
            request.id,
            ErrorCode::InvalidParams as i32,
            "Expected a Falcon fix-all command with one argument".to_owned(),
        ),
        Err(error) => Response::new_err(
            request.id,
            ErrorCode::InvalidParams as i32,
            format!("invalid params: {error}"),
        ),
    };
    if !cancelled() {
        connection.sender.send(response.into())?;
    }
    Ok(())
}

/// Hover: surface the falcon diagnostics under the cursor (rule id + message).
fn hover(state: &LspState, params: &HoverParams, cancelled: &dyn Fn() -> bool) -> Option<Hover> {
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
        .take_while(|_| !cancelled())
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
        return publish_config_messages(connection, state);
    }
    let uris = debounce.drain();
    for (uri, diagnostics) in state.analyze_affected_many(&uris) {
        publish(connection, state, &uri, &diagnostics)?;
    }
    for (uri, diagnostics) in state.cross_file_pass() {
        publish(connection, state, &uri, &diagnostics)?;
    }
    publish_config_messages(connection, state)
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
        Some(document) => {
            let lines = falcon_diagnostics::LineIndex::new(document.text());
            (
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.format_lsp_in(&lines))
                    .collect(),
                document.version,
            )
        }
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
mod tests;

#[cfg(test)]
mod analysis_tests;
