use super::*;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub(super) struct ReadRequest {
    pub method: &'static str,
    pub uri: Arc<str>,
    pub version: Option<i32>,
}

pub(super) struct RequestState {
    pub cancelled: AtomicBool,
    pub read: Option<ReadRequest>,
}

#[derive(Default)]
pub(super) struct PendingRequests {
    by_id: HashMap<RequestId, Arc<RequestState>>,
    reads: HashMap<&'static str, HashMap<Arc<str>, RequestId>>,
}

impl PendingRequests {
    pub fn register(
        &mut self,
        request: &Request,
        version: Option<i32>,
    ) -> (Arc<RequestState>, Option<(RequestId, Arc<RequestState>)>) {
        let mut superseded = None;
        let read = readonly_document(request).map(|(method, uri)| {
            let requests = self.reads.entry(method).or_default();
            let uri = requests
                .get_key_value(uri)
                .map_or_else(|| Arc::<str>::from(uri), |(uri, _)| Arc::clone(uri));
            if let Some(id) = requests.insert(Arc::clone(&uri), request.id.clone())
                && let Some(state) = self.by_id.remove(&id)
            {
                superseded = Some((id, state));
            }
            ReadRequest {
                method,
                uri,
                version,
            }
        });
        let state = Arc::new(RequestState {
            cancelled: AtomicBool::new(false),
            read,
        });
        self.by_id.insert(request.id.clone(), Arc::clone(&state));
        (state, superseded)
    }

    pub fn remove(&mut self, id: &RequestId) -> Option<Arc<RequestState>> {
        let state = self.by_id.remove(id)?;
        if let Some(read) = &state.read
            && let Some(requests) = self.reads.get_mut(read.method)
        {
            requests.remove(read.uri.as_ref());
        }
        Some(state)
    }

    pub fn contains_key(&self, id: &RequestId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (RequestId, Arc<RequestState>)> {
        self.reads.clear();
        self.by_id.drain()
    }
}

fn readonly_document(request: &Request) -> Option<(&'static str, &str)> {
    let method = match request.method.as_str() {
        HoverRequest::METHOD => HoverRequest::METHOD,
        "textDocument/codeAction" => "textDocument/codeAction",
        _ => return None,
    };
    let uri = request.params["textDocument"]["uri"].as_str()?;
    Uri::from_str(uri).ok()?;
    Some((method, uri))
}

pub(super) fn remember_document_version(versions: &mut HashMap<String, i32>, message: &Message) {
    let Message::Notification(notification) = message else {
        return;
    };
    let document = &notification.params["textDocument"];
    let Some(uri) = document["uri"].as_str() else {
        return;
    };
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD | DidChangeTextDocument::METHOD => {
            if let Some(version) = document["version"]
                .as_i64()
                .and_then(|version| i32::try_from(version).ok())
            {
                if let Some(previous) = versions.get_mut(uri) {
                    *previous = version;
                } else if notification.method == DidOpenTextDocument::METHOD {
                    versions.insert(uri.to_owned(), version);
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            versions.remove(uri);
        }
        _ => {}
    }
}

fn is_read_request((_, message, token): &WorkerMessage) -> bool {
    let Message::Request(request) = message else {
        return false;
    };
    token.as_ref().map_or_else(
        || readonly_document(request).is_some(),
        |token| token.read.is_some(),
    )
}

#[derive(Clone, Copy)]
enum DocumentEvent {
    Open,
    Change,
    Close,
    Save,
}

struct PendingDocument {
    initially_open: bool,
    close: Option<u64>,
    save: Option<u64>,
    latest: u64,
}

#[derive(Default)]
struct PendingWatcher {
    sequence: u64,
    paths: HashMap<String, usize>,
}

#[derive(Default)]
struct PendingWork {
    sequence: u64,
    work: BTreeMap<u64, WorkerMessage>,
    documents: HashMap<String, PendingDocument>,
    watcher: Option<PendingWatcher>,
    requests: HashMap<RequestId, u64>,
}

impl PendingWork {
    fn push(&mut self, mut message: WorkerMessage) {
        self.sequence += 1;
        let sequence = self.sequence;
        if let Some((uri, event)) = foldable_document(&message.1) {
            let (uri, document) = self.documents.remove_entry(uri).unwrap_or_else(|| {
                (
                    uri.to_owned(),
                    PendingDocument {
                        initially_open: !matches!(event, DocumentEvent::Open),
                        close: None,
                        save: None,
                        latest: sequence,
                    },
                )
            });
            self.fold_document(uri, document, sequence, event, &mut message);
            self.work.insert(sequence, message);
            return;
        }
        if watcher_events(&message.1).is_some()
            && let Message::Notification(mut notification) = message.1
        {
            let mut watcher = self.watcher.take().unwrap_or_default();
            let events = notification.params["changes"]
                .as_array_mut()
                .map_or_default(std::mem::take);
            if let Some((_, Message::Notification(previous), _)) =
                self.work.remove(&watcher.sequence)
            {
                notification = previous;
            }
            if let Some(changes) = notification.params["changes"].as_array_mut() {
                for event in events {
                    if let Some(uri) = event["uri"].as_str() {
                        if let Some(&index) = watcher.paths.get(uri) {
                            if event["type"] != 2 || changes[index]["type"] == 2 {
                                changes[index] = event;
                            }
                        } else {
                            watcher.paths.insert(uri.to_owned(), changes.len());
                            changes.push(event);
                        }
                    }
                }
            }
            watcher.sequence = sequence;
            self.watcher = Some(watcher);
            message.1 = notification.into();
            self.work.insert(sequence, message);
            return;
        }
        if !is_read_request(&message) {
            self.documents.clear();
            self.watcher = None;
        }
        if let Message::Request(request) = &message.1 {
            self.requests.insert(request.id.clone(), sequence);
        }
        self.work.insert(sequence, message);
    }

    fn fold_document(
        &mut self,
        uri: String,
        mut document: PendingDocument,
        sequence: u64,
        event: DocumentEvent,
        message: &mut WorkerMessage,
    ) {
        let mut latest = sequence;
        if let Some((previous_generation, Message::Notification(mut previous), _)) =
            self.work.remove(&document.latest)
        {
            if previous.method == DidSaveTextDocument::METHOD
                && !matches!(event, DocumentEvent::Save)
            {
                previous.params["text"] = serde_json::Value::Null;
                document.save = Some(document.latest);
                self.work.insert(
                    document.latest,
                    (previous_generation, previous.into(), None),
                );
            } else {
                match event {
                    DocumentEvent::Open
                        if document.initially_open
                            && previous.method == DidCloseTextDocument::METHOD =>
                    {
                        document.close = Some(document.latest);
                        self.work.insert(
                            document.latest,
                            (previous_generation, previous.into(), None),
                        );
                    }
                    DocumentEvent::Change => {
                        if let Message::Notification(next) = &mut message.1 {
                            if previous.method == DidOpenTextDocument::METHOD {
                                previous.params["textDocument"]["version"] =
                                    next.params["textDocument"]["version"].take();
                                if let Some(change) = next.params["contentChanges"]
                                    .as_array_mut()
                                    .and_then(|changes| changes.last_mut())
                                {
                                    previous.params["textDocument"]["text"] = change["text"].take();
                                }
                                message.1 = previous.into();
                            } else if previous.method == DidCloseTextDocument::METHOD {
                                message.1 = previous.into();
                            }
                        }
                    }
                    DocumentEvent::Save => {
                        if let Message::Notification(next) = &mut message.1 {
                            if previous.method == DidSaveTextDocument::METHOD {
                                if next.params["text"].is_null() {
                                    next.params["text"] = previous.params["text"].take();
                                }
                            } else {
                                if next.params["text"].is_string() {
                                    let text = next.params["text"].take();
                                    if previous.method == DidOpenTextDocument::METHOD {
                                        previous.params["textDocument"]["text"] = text;
                                    } else if previous.method == DidChangeTextDocument::METHOD
                                        && let Some(change) = previous.params["contentChanges"]
                                            .as_array_mut()
                                            .and_then(|changes| changes.last_mut())
                                    {
                                        change["text"] = text;
                                    }
                                }
                                latest = document.latest;
                                self.work
                                    .insert(latest, (previous_generation, previous.into(), None));
                                if let Some(save) = document.save.replace(sequence) {
                                    self.work.remove(&save);
                                }
                            }
                        }
                    }
                    DocumentEvent::Close | DocumentEvent::Open => {}
                }
            }
            if matches!(event, DocumentEvent::Close)
                && let Some(close) = document.close.take()
            {
                self.work.remove(&close);
            }
        }
        document.latest = latest;
        self.documents.insert(uri, document);
    }

    fn next_key(&self) -> Option<u64> {
        let (&first, message) = self.work.first_key_value()?;
        Some(if is_read_request(message) {
            self.work
                .iter()
                .skip(1)
                .find(|(_, message)| !is_read_request(message))
                .filter(|(_, (_, message, _))| matches!(message, Message::Notification(_)))
                .map_or(first, |(&key, _)| key)
        } else {
            first
        })
    }

    fn pop(&mut self) -> Option<WorkerMessage> {
        let key = self.next_key()?;
        let message = self.work.remove(&key)?;
        if let Message::Request(request) = &message.1 {
            self.requests.remove(&request.id);
        }
        if self
            .watcher
            .as_ref()
            .is_some_and(|watcher| watcher.sequence == key)
        {
            self.watcher = None;
        }
        if let Some((uri, event)) = foldable_document(&message.1)
            && let Some(document) = self.documents.get_mut(uri)
        {
            if document.latest == key {
                match event {
                    DocumentEvent::Open => document.initially_open = true,
                    DocumentEvent::Close => document.initially_open = false,
                    DocumentEvent::Change | DocumentEvent::Save => {}
                }
                if let Some(save) = document.save.take() {
                    document.latest = save;
                } else {
                    self.documents.remove(uri);
                }
            } else if document.close == Some(key) {
                document.close = None;
                document.initially_open = false;
            } else if document.save == Some(key) {
                document.save = None;
            }
        }
        Some(message)
    }
}

fn watcher_events(message: &Message) -> Option<&Vec<serde_json::Value>> {
    let Message::Notification(notification) = message else {
        return None;
    };
    if notification.method != DidChangeWatchedFiles::METHOD {
        return None;
    }
    let changes = notification.params["changes"].as_array()?;
    changes
        .iter()
        .all(|event| {
            event["uri"]
                .as_str()
                .is_some_and(|uri| Uri::from_str(uri).is_ok())
                && matches!(event["type"].as_u64(), Some(1..=3))
        })
        .then_some(changes)
}

fn foldable_document(message: &Message) -> Option<(&str, DocumentEvent)> {
    let Message::Notification(notification) = message else {
        return None;
    };
    let document = &notification.params["textDocument"];
    let uri = document["uri"].as_str()?;
    Uri::from_str(uri).ok()?;
    let event = match notification.method.as_str() {
        DidCloseTextDocument::METHOD => DocumentEvent::Close,
        DidSaveTextDocument::METHOD
            if notification.params["text"].is_null() || notification.params["text"].is_string() =>
        {
            DocumentEvent::Save
        }
        DidOpenTextDocument::METHOD
            if document["languageId"].is_string() && document["text"].is_string() =>
        {
            i32::try_from(document["version"].as_i64()?).ok()?;
            DocumentEvent::Open
        }
        DidChangeTextDocument::METHOD => {
            i32::try_from(document["version"].as_i64()?).ok()?;
            let changes = notification.params["contentChanges"].as_array()?;
            if changes.is_empty()
                || changes.iter().any(|change| {
                    !change["text"].is_string()
                        || !change["range"].is_null()
                        || !change["rangeLength"].is_null()
                })
            {
                return None;
            }
            DocumentEvent::Change
        }
        _ => return None,
    };
    Some((uri, event))
}

pub(super) struct WorkerInput {
    pending: Arc<Mutex<PendingWork>>,
    wake: crossbeam_channel::Sender<()>,
    closed: Arc<AtomicBool>,
}

pub(super) struct WorkerReceiver {
    pending: Arc<Mutex<PendingWork>>,
    wake: crossbeam_channel::Receiver<()>,
    closed: Arc<AtomicBool>,
}

pub(super) fn worker_channel() -> (WorkerInput, WorkerReceiver) {
    let pending = Arc::new(Mutex::new(PendingWork::default()));
    let closed = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = crossbeam_channel::bounded(1);
    (
        WorkerInput {
            pending: Arc::clone(&pending),
            wake: sender,
            closed: Arc::clone(&closed),
        },
        WorkerReceiver {
            pending,
            wake: receiver,
            closed,
        },
    )
}

impl WorkerInput {
    pub(super) fn cancel(&self, id: &RequestId) -> Result<(), ServerError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "LSP input queue poisoned")?;
        if let Some(sequence) = pending.requests.remove(id) {
            pending.work.remove(&sequence);
            let work = std::mem::take(&mut pending.work);
            pending.documents.clear();
            pending.watcher = None;
            pending.requests.clear();
            for message in work.into_values() {
                pending.push(message);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn send_unfolded(&self, message: WorkerMessage) {
        let mut pending = self.pending.lock().unwrap();
        pending.sequence += 1;
        let sequence = pending.sequence;
        pending.work.insert(sequence, message);
        self.wake.try_send(()).ok();
    }

    pub(super) fn send(&self, message: WorkerMessage) -> Result<(), ServerError> {
        self.pending
            .lock()
            .map_err(|_| "LSP input queue poisoned")?
            .push(message);
        match self.wake.try_send(()) {
            Ok(()) | Err(crossbeam_channel::TrySendError::Full(())) => Ok(()),
            Err(crossbeam_channel::TrySendError::Disconnected(())) => {
                Err("LSP worker disconnected".into())
            }
        }
    }
}

impl Drop for WorkerInput {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl WorkerReceiver {
    pub(super) fn recv(&self) -> Result<WorkerMessage, RecvTimeoutError> {
        self.receive(None)
    }

    pub(super) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WorkerMessage, RecvTimeoutError> {
        self.receive(Some(Instant::now() + timeout))
    }

    pub(super) fn acknowledge(
        &self,
        acknowledged: &crossbeam_channel::Receiver<bool>,
    ) -> Option<bool> {
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return None;
            }
            crossbeam_channel::select! {
                recv(acknowledged) -> current => return current.ok(),
                recv(self.wake) -> signal => if signal.is_err() { return None; },
            }
        }
    }

    fn receive(&self, deadline: Option<Instant>) -> Result<WorkerMessage, RecvTimeoutError> {
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return Err(RecvTimeoutError::Disconnected);
            }
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| RecvTimeoutError::Disconnected)?;
            if let Some(message) = pending.pop() {
                return Ok(message);
            }
            drop(pending);
            if let Some(deadline) = deadline {
                self.wake
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
            } else {
                self.wake
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected)?;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.pending.lock().unwrap().work.len()
    }

    #[cfg(test)]
    pub(super) fn try_iter(&self) -> impl Iterator<Item = WorkerMessage> + '_ {
        std::iter::from_fn(|| self.pending.lock().unwrap().pop())
    }
}
