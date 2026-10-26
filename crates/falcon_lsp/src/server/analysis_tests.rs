//! Exercise the production transport while analysis completion is controlled by
//! channels, rather than relying on a large fixture or a timing-sensitive sleep.

use super::*;
use crossbeam_channel::{Receiver, Sender};
use serde_json::json;
use std::thread::{self, JoinHandle};

const TIMEOUT: Duration = Duration::from_secs(5);

struct RelayClient {
    connection: Connection,
    inputs: Receiver<WorkerMessage>,
    rounds: Sender<AnalysisRound>,
    thread: JoinHandle<()>,
}

impl RelayClient {
    fn start() -> Self {
        let (server, connection) = Connection::memory();
        let (input, inputs) = crossbeam_channel::unbounded();
        let (rounds, output) = crossbeam_channel::unbounded();
        let thread = thread::spawn(move || relay(&server, input, output).unwrap());
        Self {
            connection,
            inputs,
            rounds,
            thread,
        }
    }

    fn forward(&self, message: Message) -> u64 {
        self.connection.sender.send(message).unwrap();
        self.inputs.recv_timeout(TIMEOUT).unwrap().0
    }

    fn request(&self, id: i32) -> u64 {
        self.forward(Request::new(id.into(), HoverRequest::METHOD.to_string(), json!({})).into())
    }

    fn edit(&self, version: i32) -> u64 {
        self.forward(
            ServerNotification::new(
                DidChangeTextDocument::METHOD.to_string(),
                json!({
                    "textDocument": { "uri": "file:///changed.dart", "version": version },
                    "contentChanges": [{ "text": "void main() {}" }]
                }),
            )
            .into(),
        )
    }

    fn complete(&self, generation: u64, settled: bool, messages: Vec<Message>) {
        let deferred = if settled {
            HashSet::new()
        } else {
            HashSet::from(["file:///changed.dart".to_string()])
        };
        self.rounds
            .send(AnalysisRound {
                generation,
                deferred: Arc::new(deferred),
                messages,
            })
            .unwrap();
    }

    fn recv(&self) -> Message {
        self.connection.receiver.recv_timeout(TIMEOUT).unwrap()
    }

    fn finish(self) {
        drop(self.connection);
        drop(self.rounds);
        self.thread.join().unwrap();
    }
}

fn diagnostics(uri: &str, version: i32) -> Message {
    ServerNotification::new(
        PublishDiagnostics::METHOD.to_string(),
        json!({
            "uri": uri, "version": version, "diagnostics": []
        }),
    )
    .into()
}

#[test]
fn cancellation_responds_while_analysis_is_blocked_and_suppresses_late_success() {
    let client = RelayClient::start();
    let generation = client.request(1);
    // No round has completed: the analysis worker cannot yet have produced a
    // hover response, but cancellation must already reach the consumer.
    client
        .connection
        .sender
        .send(
            ServerNotification::new(
                "$/cancelRequest".to_string(),
                json!({
                    "id": 1
                }),
            )
            .into(),
        )
        .unwrap();
    let Message::Response(cancelled) = client.recv() else {
        panic!("expected cancellation response")
    };
    assert_eq!(cancelled.id, RequestId::from(1));
    assert_eq!(
        cancelled.error.unwrap().code,
        ErrorCode::RequestCanceled as i32
    );

    assert_eq!(client.request(2), generation);
    client.complete(
        generation,
        true,
        vec![
            Response::new_ok(1.into(), json!("late hover")).into(),
            Response::new_ok(2.into(), json!(null)).into(),
        ],
    );
    let Message::Response(barrier) = client.recv() else {
        panic!("expected barrier response")
    };
    assert_eq!(
        barrier.id,
        RequestId::from(2),
        "cancelled request must never respond twice"
    );
    client.finish();
}

#[test]
fn newer_edits_suppress_stale_rounds_and_publish_each_file_only_once() {
    let client = RelayClient::start();
    let old = client.edit(1);
    // The second input reaches the transport before the first analysis finishes.
    let current = client.edit(2);
    assert!(current > old);
    assert_eq!(client.request(3), current);
    client.complete(
        old,
        true,
        vec![
            diagnostics("file:///unrelated.dart", 1),
            diagnostics("file:///changed.dart", 1),
        ],
    );
    // A debounce-pending generation must not flush the buffered stale result.
    client.complete(current, false, vec![]);
    client.complete(
        current,
        true,
        vec![
            diagnostics("file:///changed.dart", 1),
            diagnostics("file:///changed.dart", 2),
            Response::new_ok(3.into(), json!(null)).into(),
        ],
    );

    let mut published = Vec::new();
    loop {
        match client.recv() {
            Message::Notification(notification) => {
                assert_eq!(notification.method, PublishDiagnostics::METHOD);
                published.push(
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap(),
                );
            }
            Message::Response(response) => {
                assert_eq!(response.id, RequestId::from(3));
                break;
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }
    published.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
    assert_eq!(
        published.len(),
        2,
        "one merged publication per file: {published:?}"
    );
    assert_eq!(published[0].uri.as_str(), "file:///changed.dart");
    assert_eq!(
        published[0].version,
        Some(2),
        "never expose the superseded document version"
    );
    assert_eq!(published[1].uri.as_str(), "file:///unrelated.dart");
    assert_eq!(
        published[1].version,
        Some(1),
        "superseding another file must not lose this result"
    );
    client.finish();
}

#[test]
fn ignored_newer_notification_does_not_strand_an_unrelated_publication() {
    let client = RelayClient::start();
    let old = client.edit(1);
    // An environment-document edit is ignored by the worker, but still advances
    // transport generation and must acknowledge that generation as settled.
    let current = client.forward(
        ServerNotification::new(
            DidChangeTextDocument::METHOD.to_string(),
            json!({
                "textDocument": { "uri": "file:///falcon.json", "version": 1 },
                "contentChanges": [{ "text": "{}" }]
            }),
        )
        .into(),
    );
    assert_eq!(client.request(4), current);
    client.complete(old, true, vec![diagnostics("file:///unrelated.dart", 1)]);
    client.complete(current, true, vec![]);
    client.complete(
        current,
        true,
        vec![Response::new_ok(4.into(), json!(null)).into()],
    );
    let Message::Notification(published) = client.recv() else {
        panic!("expected retained publication")
    };
    assert_eq!(published.method, PublishDiagnostics::METHOD);
    assert_eq!(published.params["uri"], "file:///unrelated.dart");
    let Message::Response(barrier) = client.recv() else {
        panic!("expected barrier response")
    };
    assert_eq!(barrier.id, RequestId::from(4));
    client.finish();
}

#[test]
fn close_clear_stays_before_dependent_publications() {
    let client = RelayClient::start();
    let generation = client.forward(
        ServerNotification::new(
            DidCloseTextDocument::METHOD.to_string(),
            json!({
                "textDocument": { "uri": "file:///closed.dart" }
            }),
        )
        .into(),
    );
    client.complete(
        generation,
        true,
        vec![
            diagnostics("file:///closed.dart", 1),
            diagnostics("file:///dependent.dart", 1),
        ],
    );
    for uri in ["file:///closed.dart", "file:///dependent.dart"] {
        let Message::Notification(notification) = client.recv() else {
            panic!("expected publication")
        };
        assert_eq!(notification.params["uri"], uri);
    }
    client.finish();
}

#[test]
fn pending_debounce_does_not_delay_unrelated_open_save_or_close() {
    let client = RelayClient::start();
    let generation = client.edit(2);
    client.complete(
        generation,
        false,
        vec![
            diagnostics("file:///changed.dart", 1),
            diagnostics("file:///opened.dart", 1),
            diagnostics("file:///saved.dart", 2),
            diagnostics("file:///closed.dart", 1),
        ],
    );
    for uri in [
        "file:///opened.dart",
        "file:///saved.dart",
        "file:///closed.dart",
    ] {
        let Message::Notification(notification) = client.recv() else {
            panic!("expected immediate publication")
        };
        assert_eq!(notification.params["uri"], uri);
    }
    client.complete(
        generation,
        true,
        vec![diagnostics("file:///changed.dart", 2)],
    );
    let Message::Notification(notification) = client.recv() else {
        panic!("expected latest changed document")
    };
    assert_eq!(notification.params["uri"], "file:///changed.dart");
    assert_eq!(notification.params["version"], 2);
    client.finish();
}

#[test]
fn shutdown_resolves_pending_requests_before_its_response() {
    let client = RelayClient::start();
    client.request(1);
    client
        .connection
        .sender
        .send(Request::new(2.into(), "shutdown".to_string(), ()).into())
        .unwrap();
    let Message::Response(cancelled) = client.recv() else {
        panic!("expected cancellation response")
    };
    assert_eq!(cancelled.id, RequestId::from(1));
    assert_eq!(
        cancelled.error.unwrap().code,
        ErrorCode::RequestCanceled as i32
    );
    let Message::Response(shutdown) = client.recv() else {
        panic!("expected shutdown response")
    };
    assert_eq!(shutdown.id, RequestId::from(2));
    assert!(shutdown.error.is_none());
    client
        .connection
        .sender
        .send(ServerNotification::new("exit".to_string(), ()).into())
        .unwrap();
    client.finish();
}

#[test]
fn real_worker_cancels_and_supersedes_before_an_ignored_notification() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    std::fs::write(
        &config,
        r#"{
        "linter": { "rules": { "recommended": false } },
        "cross-file": { "rules": { "recommended": false } }
    }"#,
    )
    .unwrap();
    let state = LspState::new(Some(config.clone()));
    let options = ServerOptions {
        debounce: Duration::ZERO,
        config_path: Some(config),
    };
    let (server, client) = Connection::memory();
    let (input, receiver) = crossbeam_channel::unbounded();
    let (sender, output) = crossbeam_channel::unbounded();
    let (started, blocked) = crossbeam_channel::bounded(1);
    let (release, resume) = crossbeam_channel::bounded(1);
    let request_runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let worker_request_runs = Arc::clone(&request_runs);
    let worker = thread::spawn(move || {
        analysis_worker_with(options, state, FileWatching::new(&json!({})), receiver, sender, |message| {
            if matches!(message, Message::Request(request) if request.id == RequestId::from(1)) {
                worker_request_runs.fetch_add(1, Ordering::Relaxed);
            }
            if matches!(message, Message::Notification(notification) if notification.method == DidOpenTextDocument::METHOD) {
                started.send(()).unwrap();
                resume.recv_timeout(TIMEOUT).unwrap();
            }
        }).unwrap();
    });
    let transport = thread::spawn(move || relay(&server, input, output).unwrap());
    let uri = format!("file://{}", temp.path().join("changed.dart").display());
    client.sender.send(ServerNotification::new(DidOpenTextDocument::METHOD.to_string(), json!({
        "textDocument": { "uri": uri, "languageId": "dart", "version": 1, "text": "void oldName() {}" }
    })).into()).unwrap();
    blocked.recv_timeout(TIMEOUT).unwrap();
    client
        .sender
        .send(
            Request::new(
                1.into(),
                HoverRequest::METHOD.to_string(),
                json!({
                    "textDocument": { "uri": uri }, "position": { "line": 0, "character": 0 }
                }),
            )
            .into(),
        )
        .unwrap();
    client
        .sender
        .send(
            ServerNotification::new(
                DidChangeTextDocument::METHOD.to_string(),
                json!({
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "text": "void newName() {}" }]
                }),
            )
            .into(),
        )
        .unwrap();
    client
        .sender
        .send(
            ServerNotification::new(
                DidChangeTextDocument::METHOD.to_string(),
                json!({
                    "textDocument": { "uri": "file:///falcon.json", "version": 1 },
                    "contentChanges": [{ "text": "{}" }]
                }),
            )
            .into(),
        )
        .unwrap();
    client
        .sender
        .send(ServerNotification::new("$/cancelRequest".to_string(), json!({ "id": 1 })).into())
        .unwrap();
    let Message::Response(cancelled) = client.receiver.recv_timeout(TIMEOUT).unwrap() else {
        panic!("worker must not block cancellation");
    };
    assert_eq!(cancelled.id, RequestId::from(1));
    assert_eq!(
        cancelled.error.unwrap().code,
        ErrorCode::RequestCanceled as i32
    );
    // Receiving cancellation proves the relay consumed both edits while the
    // actual worker was gated; now its old and new analyses can finish.
    client
        .sender
        .send(Request::new(2.into(), "falcon/testBarrier".to_string(), ()).into())
        .unwrap();
    release.send(()).unwrap();
    let mut publications = Vec::new();
    loop {
        match client.receiver.recv_timeout(TIMEOUT).unwrap() {
            Message::Notification(notification) => {
                assert_eq!(notification.method, PublishDiagnostics::METHOD);
                publications.push(
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap(),
                );
            }
            Message::Response(response) => {
                assert_eq!(
                    response.id,
                    RequestId::from(2),
                    "no late cancelled hover response"
                );
                break;
            }
            message => panic!("unexpected worker output: {message:?}"),
        }
    }
    assert_eq!(
        publications.len(),
        1,
        "no superseded or preliminary publications: {publications:?}"
    );
    assert_eq!(publications[0].uri.as_str(), uri);
    assert_eq!(publications[0].version, Some(2));
    drop(client);
    transport.join().unwrap();
    worker.join().unwrap();
    assert_eq!(
        request_runs.load(Ordering::Relaxed),
        0,
        "a cancelled queued request must never enter the worker handler"
    );
}

#[test]
fn debounce_snapshots_share_storage_until_dirty_membership_changes() {
    let mut debounce = Debounce::default();
    debounce.schedule("file:///first.dart".to_string(), Duration::from_secs(1));
    let first = Arc::clone(&debounce.dirty);
    debounce.schedule("file:///first.dart".to_string(), Duration::from_secs(1));
    debounce.clear("file:///absent.dart");
    assert!(
        Arc::ptr_eq(&first, &debounce.dirty),
        "repeated edits and unrelated clears must not clone the dirty set"
    );

    debounce.schedule("file:///second.dart".to_string(), Duration::from_secs(1));
    assert!(!Arc::ptr_eq(&first, &debounce.dirty));
    assert_eq!(
        first.len(),
        1,
        "earlier in-flight round keeps its original snapshot"
    );
    let both = Arc::clone(&debounce.dirty);
    debounce.clear("file:///second.dart");
    assert_eq!(both.len(), 2);
    assert_eq!(debounce.dirty.len(), 1);

    let pending = Arc::clone(&debounce.dirty);
    assert_eq!(debounce.drain(), vec!["file:///first.dart".to_string()]);
    assert!(debounce.dirty.is_empty());
    assert_eq!(
        pending.len(),
        1,
        "draining must not mutate an in-flight snapshot"
    );
}
