use super::*;
use serde_json::json;
use std::path::Path;

const TIMEOUT: Duration = Duration::from_secs(5);

fn document_event(method: &str, uri: &str, version: i32, text: &str) -> Message {
    let params = match method {
        DidOpenTextDocument::METHOD => {
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": version, "text": text}})
        }
        DidCloseTextDocument::METHOD => json!({"textDocument": {"uri": uri}}),
        DidSaveTextDocument::METHOD => json!({"textDocument": {"uri": uri}, "text": text}),
        _ => {
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]})
        }
    };
    ServerNotification::new(method.to_owned(), params).into()
}

fn enqueue(input: &WorkerInput, messages: &[Message], folded: bool) {
    enqueue_at(input, messages, folded, 0);
}

fn enqueue_at(input: &WorkerInput, messages: &[Message], folded: bool, offset: usize) {
    for (index, message) in messages.iter().enumerate() {
        let message = ((offset + index) as u64 + 1, message.clone(), None);
        if folded {
            input.send(message).unwrap();
        } else {
            input.send_unfolded(message);
        }
    }
}

#[test]
fn lifecycle_bursts_retain_at_most_two_transitions_per_document() {
    for initially_open in [false, true] {
        let (input, receiver) = worker_channel();
        for version in 1..=4_096 {
            let uri = format!("file:///burst-{}.dart", version % 64);
            for method in if initially_open {
                [DidCloseTextDocument::METHOD, DidOpenTextDocument::METHOD]
            } else {
                [DidOpenTextDocument::METHOD, DidCloseTextDocument::METHOD]
            } {
                input
                    .send((
                        u64::try_from(version).unwrap(),
                        document_event(method, &uri, version, "class Latest {}"),
                        None,
                    ))
                    .unwrap();
            }
        }
        let expected = if initially_open { 128 } else { 64 };
        assert_eq!(receiver.len(), expected);
        println!(
            "burst=lifecycle initially_open={initially_open} events=8192 documents=64 input_depth={expected}"
        );
        let messages: Vec<_> = receiver.try_iter().collect();
        assert_eq!(messages.len(), expected);
        if initially_open {
            for pair in messages.as_chunks::<2>().0 {
                assert!(
                    matches!(&pair[0].1, Message::Notification(notification) if notification.method == DidCloseTextDocument::METHOD)
                );
                assert!(
                    matches!(&pair[1].1, Message::Notification(notification) if notification.method == DidOpenTextDocument::METHOD)
                );
            }
        }
    }
}

#[test]
fn folding_preserves_received_versions_and_observable_barriers() {
    let (input, receiver) = worker_channel();
    let a = "file:///a.dart";
    let b = "file:///b.dart";
    let changes = vec![
        document_event(DidChangeTextDocument::METHOD, a, 10, "old"),
        document_event(DidChangeTextDocument::METHOD, b, 4, "other"),
        document_event(DidChangeTextDocument::METHOD, a, 2, "latest received"),
        Request::new(1.into(), "falcon/barrier".to_owned(), ()).into(),
        document_event(DidCloseTextDocument::METHOD, a, 0, ""),
        ServerNotification::new(
            DidChangeWatchedFiles::METHOD.to_owned(),
            json!({"changes": [{"uri": a, "type": 2}]}),
        )
        .into(),
        document_event(DidOpenTextDocument::METHOD, a, 20, "reopened"),
        document_event(DidChangeTextDocument::METHOD, a, 21, "newest"),
    ];
    enqueue(&input, &changes, true);
    let messages: Vec<_> = receiver.try_iter().map(|(_, message, _)| message).collect();
    assert_eq!(messages.len(), 6);
    assert!(
        matches!(&messages[0], Message::Notification(notification) if notification.params["textDocument"]["uri"] == b)
    );
    assert!(
        matches!(&messages[1], Message::Notification(notification) if notification.params["textDocument"]["version"] == 2)
    );
    assert!(matches!(&messages[2], Message::Request(_)));
    assert!(
        matches!(&messages[3], Message::Notification(notification) if notification.method == DidCloseTextDocument::METHOD)
    );
    assert!(
        matches!(&messages[4], Message::Notification(notification) if notification.method == DidChangeWatchedFiles::METHOD)
    );
    assert!(
        matches!(&messages[5], Message::Notification(notification) if notification.method == DidOpenTextDocument::METHOD && notification.params["textDocument"]["version"] == 21 && notification.params["textDocument"]["text"] == "newest")
    );
}

#[test]
fn incremental_and_malformed_changes_are_lossless_barriers() {
    let (input, receiver) = worker_channel();
    let full = document_event(DidChangeTextDocument::METHOD, "file:///a.dart", 1, "full");
    let incremental: Message = ServerNotification::new(DidChangeTextDocument::METHOD.to_owned(), json!({
        "textDocument": {"uri": "file:///a.dart", "version": 2},
        "contentChanges": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}, "text": "part"}]
    })).into();
    let malformed: Message = ServerNotification::new(
        DidChangeTextDocument::METHOD.to_owned(),
        json!({
            "textDocument": {"uri": "file:///a.dart", "version": "invalid"}, "contentChanges": []
        }),
    )
    .into();
    let messages = vec![full.clone(), incremental, full.clone(), malformed, full];
    enqueue(&input, &messages, true);
    assert_eq!(receiver.len(), messages.len());
    let retained: Vec<_> = receiver
        .try_iter()
        .map(|(_, message, _)| serde_json::to_value(message).unwrap())
        .collect();
    assert_eq!(
        retained,
        messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>()
    );
}

fn replay(
    root: &Path,
    messages: &[Message],
    folded: bool,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    replay_interleaved(root, messages, folded, &[messages.len()])
}

fn replay_interleaved(
    root: &Path,
    messages: &[Message],
    folded: bool,
    batches: &[usize],
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let config = root.join("falcon.json");
    let mut state = LspState::new_with_sdk_locator(Some(config.clone()), |_| None);
    state.initialize_workspace_roots(vec![root.to_owned()]);
    state.set_dynamic_file_watching(false);
    let options = ServerOptions {
        debounce: Duration::ZERO,
        config_path: Some(config),
    };
    let (input, receiver) = worker_channel();
    let mut batches = batches.iter().copied();
    let mut sent = batches.next().unwrap().min(messages.len());
    enqueue(&input, &messages[..sent], folded);
    let (sender, output) = crossbeam_channel::bounded(0);
    let worker = std::thread::spawn(move || {
        analysis_worker_with(
            &options,
            state,
            FileWatching::new(&json!({})),
            &receiver,
            &sender,
            |_| {},
        )
        .unwrap();
    });
    let mut pending = PendingRequests::default();
    let mut publications = HashMap::new();
    let mut other = Vec::new();
    let mut order = 0;
    let mut warnings = Vec::new();
    let mut peak_round_messages = 0;
    let mut peak_round_bytes = 0;
    let mut peak_publications = 0;
    let mut peak_publication_bytes = 0;
    assert_eq!(output.capacity(), Some(0));
    loop {
        let round = output.recv_timeout(TIMEOUT).unwrap();
        if round.generation > 0 && sent < messages.len() {
            let end = (sent + batches.next().unwrap_or(1)).min(messages.len());
            enqueue_at(&input, &messages[sent..end], folded, sent);
            sent = end;
        }
        let current = round.generation == messages.len() as u64;
        peak_round_messages = peak_round_messages.max(round.messages.len());
        peak_round_bytes = peak_round_bytes.max(
            round
                .messages
                .iter()
                .map(|message| {
                    std::mem::size_of::<Message>() + serde_json::to_vec(message).unwrap().len()
                })
                .sum::<usize>(),
        );
        buffer_round_messages(
            round.messages,
            current,
            &mut pending,
            &mut publications,
            &mut other,
            &mut order,
        )
        .unwrap();
        peak_publications = peak_publications.max(publications.len());
        peak_publication_bytes = peak_publication_bytes.max(
            publications
                .values()
                .map(|(_, notification)| {
                    std::mem::size_of::<ServerNotification>()
                        + serde_json::to_vec(notification).unwrap().len()
                })
                .sum::<usize>(),
        );
        warnings.extend(other.drain(..).map(|message| {
            let Message::Notification(notification) = message else {
                panic!("unexpected output: {message:?}");
            };
            assert_eq!(notification.method, ShowMessage::METHOD);
            notification.params
        }));
        if current && let Some(message) = round.incomplete_message {
            warnings.push(serde_json::to_value(message).unwrap());
        }
        round.acknowledgement.unwrap().try_send(current).unwrap();
        if current {
            assert!(round.deferred.is_empty());
            break;
        }
    }
    drop(input);
    drop(output);
    worker.join().unwrap();
    println!(
        "worker folded={folded} events={} completed_round_queue=0 round_messages_peak={peak_round_messages} round_wire_plus_inline_bytes={peak_round_bytes} relay_publications_peak={peak_publications} publication_wire_plus_inline_bytes={peak_publication_bytes}",
        messages.len()
    );
    let mut publications: Vec<_> = publications
        .into_values()
        .map(|(_, notification)| notification.params)
        .collect();
    publications.sort_by(|a, b| a["uri"].as_str().cmp(&b["uri"].as_str()));
    (publications, warnings)
}

#[test]
fn stale_config_warning_survives_a_following_document_open() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let config = root.join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"domains":{"fluter":"recommended"}}}"#,
    )
    .unwrap();
    let uri = format!("file://{}", root.join("main.dart").display());
    let messages = [
        ServerNotification::new(
            DidChangeWatchedFiles::METHOD.to_owned(),
            json!({"changes": [{"uri": format!("file://{}", config.display()), "type": 2}]}),
        )
        .into(),
        document_event(DidOpenTextDocument::METHOD, &uri, 1, "void main() {}"),
    ];
    for folded in [false, true] {
        let (_, warnings) = replay(root, &messages, folded);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]["message"]
                .as_str()
                .unwrap()
                .contains("unknown domain `linter.domains.fluter`"),
            "{warnings:?}"
        );
    }
}

#[test]
fn popping_open_with_pending_save_preserves_close_and_reopen() {
    let (input, receiver) = worker_channel();
    let uri = "file:///main.dart";
    enqueue(
        &input,
        &[
            document_event(DidOpenTextDocument::METHOD, uri, 1, "class First {}"),
            document_event(DidSaveTextDocument::METHOD, uri, 1, "class Saved {}"),
        ],
        true,
    );
    assert!(
        matches!(receiver.recv_timeout(TIMEOUT).unwrap().1, Message::Notification(notification) if notification.method == DidOpenTextDocument::METHOD)
    );
    enqueue(
        &input,
        &[
            document_event(DidCloseTextDocument::METHOD, uri, 0, ""),
            document_event(DidOpenTextDocument::METHOD, uri, 2, "class Reopened {}"),
        ],
        true,
    );
    let methods: Vec<_> = receiver
        .try_iter()
        .map(|(_, message, _)| {
            let Message::Notification(notification) = message else {
                panic!("expected notification");
            };
            notification.method
        })
        .collect();
    assert_eq!(
        methods,
        [
            DidSaveTextDocument::METHOD,
            DidCloseTextDocument::METHOD,
            DidOpenTextDocument::METHOD
        ]
    );
}

#[test]
fn folded_and_unfolded_runs_match_final_diagnostics_and_live_episodes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("workspace");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("falcon.json"), r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#).unwrap();
    let source = "class OrphanThing {}";
    let broken = "part 'broken.dart'; class OrphanThing {}";
    std::fs::write(root.join("lib/orphan.dart"), source).unwrap();
    std::fs::write(root.join("lib/main.dart"), "void main() {}").unwrap();
    let broken_path = root.join("lib/broken.dart");
    std::fs::write(&broken_path, [0xff]).unwrap();
    let uri = format!("file://{}", root.join("lib/orphan.dart").display());
    let transient = vec![
        document_event(DidOpenTextDocument::METHOD, &uri, 1, broken),
        document_event(DidCloseTextDocument::METHOD, &uri, 0, ""),
    ];
    let transient_unfolded = replay(&root, &transient, false);
    assert!(
        transient_unfolded.1.is_empty(),
        "transient failures must not announce a stale episode"
    );
    assert_eq!(replay(&root, &transient, true), transient_unfolded);
    for final_text in [source, broken] {
        if final_text == source {
            std::fs::remove_file(&broken_path).unwrap();
        } else {
            std::fs::write(&broken_path, [0xff]).unwrap();
        }
        let mut history = vec![document_event(DidOpenTextDocument::METHOD, &uri, 1, broken)];
        for version in 2..=65 {
            history.push(document_event(
                DidChangeTextDocument::METHOD,
                &uri,
                version,
                final_text,
            ));
        }
        history.push(document_event(DidCloseTextDocument::METHOD, &uri, 0, ""));
        history.push(document_event(
            DidOpenTextDocument::METHOD,
            &uri,
            66,
            final_text,
        ));
        for version in 67..=128 {
            history.push(document_event(
                DidChangeTextDocument::METHOD,
                &uri,
                version,
                final_text,
            ));
        }
        let unfolded = replay(&root, &history, false);
        assert_eq!(unfolded.1.len(), usize::from(final_text == broken));
        assert_eq!(replay(&root, &history, true), unfolded);
        let publication = unfolded
            .0
            .iter()
            .find(|publication| publication["uri"] == uri)
            .unwrap();
        assert_eq!(publication["version"], 128);
        let codes: Vec<_> = publication["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&if final_text == broken {
            "incomplete-analysis"
        } else {
            "unused-files"
        }));
    }
}

#[test]
fn shutdown_discards_queued_burst_without_draining_analysis() {
    let (input, receiver) = worker_channel();
    for index in 0..4_096 {
        input
            .send((
                index,
                document_event(
                    DidChangeTextDocument::METHOD,
                    "file:///a.dart",
                    i32::try_from(index).unwrap(),
                    "latest",
                ),
                None,
            ))
            .unwrap();
    }
    assert_eq!(receiver.len(), 1);
    drop(input);
    assert!(matches!(
        receiver.recv_timeout(TIMEOUT),
        Err(RecvTimeoutError::Disconnected)
    ));
    let (acknowledgement, acknowledged) = crossbeam_channel::bounded(1);
    assert_eq!(receiver.acknowledge(&acknowledged), None);
    drop(acknowledgement);
}

#[test]
fn watcher_registration_waits_for_acknowledgement_without_losing_refresh() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let state = LspState::new_with_sdk_locator(Some(config), |_| None);
    let mut watching = FileWatching::new(
        &json!({"capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true,"relativePatternSupport":true}}}}),
    );
    let (server, client) = Connection::memory();
    watching.refresh(&server, &state).unwrap();
    let Message::Request(initial) = client.receiver.recv_timeout(TIMEOUT).unwrap() else {
        panic!("expected registration");
    };
    for _ in 0..4_096 {
        watching.dependency_generation = None;
        watching.dependency_files = vec![directory.path().join("obsolete")];
        watching.refresh(&server, &state).unwrap();
    }
    assert_eq!(watching.pending.len(), 1);
    assert!(client.receiver.is_empty());
    watching.response(Response::new_ok(initial.id, ()));
    watching.refresh(&server, &state).unwrap();
    assert_eq!(watching.pending.len(), 2);
    let messages: Vec<_> = client.receiver.try_iter().collect();
    assert_eq!(messages.len(), 2);
    assert!(
        watching
            .dependency_files
            .iter()
            .all(|path| !path.ends_with("obsolete"))
    );
    println!("burst=watcher-registration refreshes=4096 pending=1 replacement_pending=2");
}

fn interleaved_history(uris: &[String], episode: &str) -> Vec<Message> {
    let mut history = Vec::new();
    let mut open = [false; 4];
    let mut seed = 42_u64;
    for version in 1..=1_024 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let index = (seed >> 32) as usize % uris.len();
        let choice = (seed >> 40) % 6;
        if choice < 2 {
            history.push(
                ServerNotification::new(
                    DidChangeWatchedFiles::METHOD.to_owned(),
                    json!({
                        "changes": [{"uri": uris[index], "type": if choice == 0 { 3 } else { 1 }}]
                    }),
                )
                .into(),
            );
        } else if choice == 5 {
            history.push(document_event(
                DidSaveTextDocument::METHOD,
                &uris[index],
                version,
                &format!("class Saved{version} {{}}"),
            ));
        } else {
            let method = if !open[index] {
                open[index] = true;
                DidOpenTextDocument::METHOD
            } else if choice == 2 {
                open[index] = false;
                DidCloseTextDocument::METHOD
            } else {
                DidChangeTextDocument::METHOD
            };
            history.push(document_event(
                method,
                &uris[index],
                version,
                &format!("class Latest{version} {{}}"),
            ));
        }
    }
    for (index, uri) in uris.iter().enumerate() {
        history.push(document_event(
            if episode == "closed" {
                DidCloseTextDocument::METHOD
            } else {
                DidOpenTextDocument::METHOD
            },
            uri,
            2_000 + i32::try_from(index).unwrap(),
            "class Final {}",
        ));
    }
    for kind in [3, 1, 2] {
        history.push(
            ServerNotification::new(
                DidChangeWatchedFiles::METHOD.to_owned(),
                json!({
                    "changes": [{"uri": uris[0], "type": kind}]
                }),
            )
            .into(),
        );
    }
    history
}

#[test]
fn interleaved_watchers_edits_and_lifecycles_match_unfolded_processing() {
    for episode in ["healthy", "persistent", "closed"] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("lib")).unwrap();
        std::fs::write(root.join("falcon.json"), r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#).unwrap();
        if episode != "healthy" {
            std::fs::write(root.join("lib/broken.dart"), [0xff]).unwrap();
        }
        let uris: Vec<_> = (0..4)
            .map(|index| {
                let path = root.join(format!("lib/doc{index}.dart"));
                std::fs::write(&path, "class OnDisk {}").unwrap();
                format!("file://{}", path.display())
            })
            .collect();
        let history = interleaved_history(&uris, episode);
        let (input, receiver) = worker_channel();
        enqueue(&input, &history, true);
        assert!(receiver.len() <= 13, "{} retained events", receiver.len());
        let retained: Vec<_> = receiver.try_iter().map(|(_, message, _)| message).collect();
        let watcher = retained
            .iter()
            .find_map(|message| match message {
                Message::Notification(notification)
                    if notification.method == DidChangeWatchedFiles::METHOD =>
                {
                    notification.params["changes"].as_array()
                }
                _ => None,
            })
            .unwrap();
        assert!(watcher.len() <= 4);
        assert_eq!(
            watcher
                .iter()
                .find(|event| event["uri"] == uris[0])
                .unwrap()["type"],
            1
        );
        let unfolded = replay(root, &history, false);
        assert_eq!(unfolded.1.len(), usize::from(episode == "persistent"));
        assert_eq!(replay(root, &history, true), unfolded, "episode={episode}");
        let mut streaming = vec![
            document_event(DidOpenTextDocument::METHOD, &uris[0], 1, "class First {}"),
            document_event(DidSaveTextDocument::METHOD, &uris[0], 1, "class Saved {}"),
            document_event(DidCloseTextDocument::METHOD, &uris[0], 0, ""),
            document_event(
                DidOpenTextDocument::METHOD,
                &uris[0],
                2,
                "class Reopened {}",
            ),
        ];
        streaming.extend(history.iter().cloned());
        let mut batches = vec![2, 2];
        let mut seed = 93_u64;
        batches.extend((0..streaming.len()).map(|_| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            ((seed >> 32) as usize % 17) + 1
        }));
        let unfolded = replay_interleaved(root, &streaming, false, &batches);
        assert_eq!(unfolded.1.len(), usize::from(episode == "persistent"));
        assert_eq!(
            replay_interleaved(root, &streaming, true, &batches),
            unfolded,
            "streaming episode={episode}"
        );
        println!(
            "burst=interleaved episode={episode} events={} documents=4 watcher_paths=4 input_depth={}",
            history.len(),
            retained.len()
        );
    }
}

#[test]
fn save_bursts_keep_latest_text_last_received_version_and_one_refresh() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("lib")).unwrap();
    let path = root.join("lib/orphan.dart");
    std::fs::write(&path, "class OnDisk {}").unwrap();
    let uri = format!("file://{}", path.display());
    let mut history = vec![
        document_event(DidOpenTextDocument::METHOD, &uri, 10, "class Open {}"),
        document_event(DidSaveTextDocument::METHOD, &uri, 0, "class Saved {}"),
        document_event(DidChangeTextDocument::METHOD, &uri, 2, "class Changed {}"),
    ];
    for _ in 0..4_096 {
        history.push(document_event(
            DidSaveTextDocument::METHOD,
            &uri,
            0,
            "class FinalSaved {}",
        ));
    }
    history.push(
        ServerNotification::new(
            DidSaveTextDocument::METHOD.to_owned(),
            json!({"textDocument": {"uri": uri}}),
        )
        .into(),
    );
    let (input, receiver) = worker_channel();
    enqueue(&input, &history, true);
    assert_eq!(receiver.len(), 2);
    let retained: Vec<_> = receiver.try_iter().map(|(_, message, _)| message).collect();
    let Message::Notification(source) = &retained[0] else {
        panic!("expected open buffer");
    };
    assert_eq!(source.method, DidOpenTextDocument::METHOD);
    assert_eq!(source.params["textDocument"]["version"], 2);
    assert_eq!(source.params["textDocument"]["text"], "class FinalSaved {}");
    let Message::Notification(save) = &retained[1] else {
        panic!("expected save refresh");
    };
    assert_eq!(save.method, DidSaveTextDocument::METHOD);
    assert!(save.params["text"].is_null());
    let unfolded = replay(root, &history, false);
    assert_eq!(replay(root, &history, true), unfolded);
    assert!(
        unfolded
            .0
            .iter()
            .any(|publication| publication["uri"] == uri && publication["version"] == 2)
    );
    println!(
        "burst=save events={} documents=1 input_depth=2",
        history.len()
    );
}

#[test]
fn cancelled_request_barriers_do_not_retain_old_document_epochs() {
    let (input, receiver) = worker_channel();
    let uri = "file:///changed.dart";
    for version in 1..=4_096 {
        input
            .send((
                version,
                document_event(
                    DidChangeTextDocument::METHOD,
                    uri,
                    i32::try_from(version).unwrap(),
                    "class Latest {}",
                ),
                None,
            ))
            .unwrap();
        let id = RequestId::from(i32::try_from(version).unwrap());
        input
            .send((
                version,
                Request::new(id.clone(), HoverRequest::METHOD.to_owned(), json!({})).into(),
                None,
            ))
            .unwrap();
        input.cancel(&id).unwrap();
        assert_eq!(receiver.len(), 1);
    }
}

fn queued_request_response(method: &str, next_version: i32) -> Response {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let config = root.join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"style":{"unnecessary-new":"warn"}}}}"#,
    )
    .unwrap();
    let uri = format!("file://{}", root.join("main.dart").display());
    let source = "void main() { new Object(); }";
    let params = if method == "codeAction/resolve" {
        json!({"title":"Fix", "data":{"uri":uri,"version":1,"includeUnsafe":false}})
    } else {
        json!({"textDocument":{"uri":uri},"position":{"line":0,"character":0}})
    };
    let request = Request::new(1.into(), method.to_owned(), params);
    let (token, _) = PendingRequests::default().register(&request, Some(1));
    let (input, receiver) = worker_channel();
    input
        .send((
            1,
            document_event(DidOpenTextDocument::METHOD, &uri, 1, source),
            None,
        ))
        .unwrap();
    input.send((1, request.into(), Some(token))).unwrap();
    input
        .send((
            2,
            document_event(DidChangeTextDocument::METHOD, &uri, next_version, source),
            None,
        ))
        .unwrap();
    let mut state = LspState::new_with_sdk_locator(Some(config.clone()), |_| None);
    state.initialize_workspace_roots(vec![root.to_owned()]);
    state.configure_code_actions(
        &json!({"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}),
    );
    let (sender, output) = crossbeam_channel::bounded(0);
    let worker = std::thread::spawn(move || {
        analysis_worker_with(
            &ServerOptions {
                debounce: Duration::ZERO,
                config_path: Some(config),
            },
            state,
            FileWatching::new(&json!({})),
            &receiver,
            &sender,
            |_| {},
        )
        .unwrap();
    });
    let mut response = None;
    loop {
        let round = output.recv_timeout(TIMEOUT).unwrap();
        for message in round.messages {
            if let Message::Response(result) = message {
                assert!(response.replace(result).is_none());
            }
        }
        round.acknowledgement.unwrap().try_send(true).unwrap();
        if response.is_some() && round.generation == 2 {
            break;
        }
    }
    drop(input);
    drop(output);
    worker.join().unwrap();
    response.unwrap()
}

#[test]
fn read_request_crosses_edits_but_rejects_an_advanced_version() {
    let response = queued_request_response(HoverRequest::METHOD, 2);
    assert_eq!(
        response.error.unwrap().code,
        ErrorCode::ContentModified as i32
    );
    assert!(
        queued_request_response(HoverRequest::METHOD, 1)
            .error
            .is_none()
    );
}

#[test]
fn resolving_an_action_keeps_its_versioned_edit_snapshot() {
    let response = queued_request_response("codeAction/resolve", 2);
    assert!(response.error.is_none(), "{response:?}");
    let result = response.result.unwrap();
    assert_eq!(
        result["edit"]["documentChanges"][0]["textDocument"]["version"],
        1
    );
    assert_eq!(
        result["edit"]["documentChanges"][0]["edits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn interleaved_close_refreshes_disk_state_and_publishes_clear_before_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("lib")).unwrap();
    let path = root.join("lib/main.dart");
    std::fs::write(&path, "class OnDisk {}").unwrap();
    let config = root.join("falcon.json");
    std::fs::write(&config, r#"{"linter":{"rules":{"recommended":false,"style":{"omit-local-variable-types":"warn"}}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#).unwrap();
    let uri = format!("file://{}", path.display());
    let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
    state.initialize_workspace_roots(vec![root.to_owned()]);
    state.set_dynamic_file_watching(false);
    let (connection, client) = Connection::memory();
    let mut debounce = Debounce::default();
    let mut watching = FileWatching::new(&json!({}));
    cross_file_pass_and_publish(&connection, &mut state, &mut debounce).unwrap();
    let (input, receiver) = worker_channel();
    enqueue(
        &input,
        &[
            document_event(DidOpenTextDocument::METHOD, &uri, 1, "class First {}"),
            document_event(DidSaveTextDocument::METHOD, &uri, 1, "class Saved {}"),
        ],
        true,
    );
    let Message::Notification(open) = receiver.recv_timeout(TIMEOUT).unwrap().1 else {
        panic!("expected open");
    };
    handle_notification(
        &connection,
        &mut state,
        &mut debounce,
        &mut watching,
        Duration::ZERO,
        open,
    )
    .unwrap();
    enqueue(
        &input,
        &[
            document_event(DidCloseTextDocument::METHOD, &uri, 0, ""),
            document_event(DidOpenTextDocument::METHOD, &uri, 2, "class Reopened {}"),
        ],
        true,
    );
    let mut closed = false;
    for (_, message, _) in receiver.try_iter() {
        let Message::Notification(notification) = message else {
            panic!("expected notification");
        };
        let close = notification.method == DidCloseTextDocument::METHOD;
        client.receiver.try_iter().for_each(drop);
        if close {
            std::fs::write(&path, "class UpdatedOnDiskAfterSave {}").unwrap();
            assert!(!state.has_closed_disk_state(&path));
        }
        handle_notification(
            &connection,
            &mut state,
            &mut debounce,
            &mut watching,
            Duration::ZERO,
            notification,
        )
        .unwrap();
        if close {
            closed = true;
            assert!(state.document(&uri).is_none());
            assert!(state.has_closed_disk_state(&path));
            assert!(client.receiver.try_iter().any(|message| {
                matches!(message, Message::Notification(notification)
                    if notification.method == PublishDiagnostics::METHOD
                    && notification.params["uri"] == uri
                    && notification.params["diagnostics"] == json!([]))
            }));
        }
    }
    assert!(closed);
    let document = state.document(&uri).unwrap();
    assert_eq!(document.version, Some(2));
    assert_eq!(document.text(), "class Reopened {}");
}
