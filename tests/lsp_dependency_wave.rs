//! The language server must survive a wide dependency export wave on hosts with
//! few processors, where Rayon's global pool has one or two small-stack workers.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Files exported by the path dependency's library, all loaded in one wave.
const EXPORTS: usize = 3000;
const RULE: &str = "avoid-ignoring-return-values";

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// What `dart pub get --offline` leaves for `app` depending on `path: ../dep`,
/// plus a minimal SDK. Only the last export declares the called function, so
/// the diagnostic below requires resolving the whole wave.
fn write_fixture(root: &Path) {
    write(
        &root.join("sdk/lib/libraries.json"),
        r#"{"vm": {"libraries": {"core": {"uri": "core/core.dart"}}}}"#,
    );
    write(
        &root.join("sdk/lib/core/core.dart"),
        "library dart.core;\nclass Object {}\nabstract final class int {}\n",
    );
    write(
        &root.join("dep/pubspec.yaml"),
        "name: dep\nenvironment:\n  sdk: ^3.5.0\n",
    );
    for index in 1..EXPORTS {
        write(
            &root.join(format!("dep/lib/src/c{index}.dart")),
            &format!("class C{index} {{}}\n"),
        );
    }
    write(
        &root.join(format!("dep/lib/src/c{EXPORTS}.dart")),
        &format!("class C{EXPORTS} {{}}\nint last() => {EXPORTS};\n"),
    );
    let mut exports = String::new();
    for index in 1..=EXPORTS {
        writeln!(exports, "export 'src/c{index}.dart';").unwrap();
    }
    write(&root.join("dep/lib/all.dart"), &exports);
    write(
        &root.join("app/pubspec.yaml"),
        "name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  dep:\n    path: ../dep\n",
    );
    write(
        &root.join("app/pubspec.lock"),
        "packages:\n  dep:\n    dependency: \"direct main\"\n    description:\n      path: \"../dep\"\n      relative: true\n    source: path\n    version: \"0.0.0\"\n",
    );
    write(
        &root.join("app/.dart_tool/package_config.json"),
        r#"{"configVersion": 2, "packages": [
            {"name": "dep", "rootUri": "../../dep", "packageUri": "lib/", "languageVersion": "3.5"},
            {"name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.5"}
        ]}"#,
    );
    write(
        &root.join("app/falcon.json"),
        &format!(
            r#"{{"linter": {{"rules": {{"recommended": false, "suspicious": {{"{RULE}": "warn"}}}}}},
                "cross-file": {{"enabled": false}}}}"#
        ),
    );
}

const MAIN: &str = "import 'package:dep/all.dart';\n\nvoid check() {\n  last();\n}\n";

/// Kills and reaps the server whenever the test stops before a clean exit.
struct Server {
    child: Child,
    messages: mpsc::Receiver<String>,
    stderr: PathBuf,
    threads: &'static str,
}

impl Server {
    fn start(app: &Path, sdk: &Path, stderr: PathBuf, threads: &'static str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_falcon"))
            .arg("lsp")
            .current_dir(app)
            .env("RAYON_NUM_THREADS", threads)
            .env("DART_SDK", sdk)
            .env_remove("FLUTTER_ROOT")
            .env("PATH", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let (sender, messages) = mpsc::channel();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        std::thread::spawn(move || {
            while let Some(message) = read_message(&mut stdout) {
                if sender.send(message).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            messages,
            stderr,
            threads,
        }
    }

    fn fail(&mut self, problem: &str) -> ! {
        let _ = self.child.kill();
        let status = self.child.wait();
        panic!(
            "RAYON_NUM_THREADS={}: {problem}; status {status:?}; stderr:\n{}",
            self.threads,
            std::fs::read_to_string(&self.stderr).unwrap_or_default()
        );
    }

    fn send(&mut self, body: &str) {
        let stdin = self.child.stdin.as_mut().unwrap();
        let sent = write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len())
            .and_then(|()| stdin.flush());
        if let Err(error) = sent {
            self.fail(&format!("sending {body} failed: {error}"));
        }
    }

    fn receive(&mut self, timeout: Duration, matches: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + timeout;
        let mut skipped = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.messages.recv_timeout(remaining) {
                Ok(message) if matches(&message) => return message,
                Ok(message) => skipped.push(message),
                Err(error) => self.fail(&format!("{error} after {skipped:#?}")),
            }
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => self.fail("the server did not exit"),
                Err(error) => self.fail(&format!("waiting failed: {error}")),
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

fn publishes_resolved_dependency_diagnostics(threads: &'static str) {
    let root = tempfile::tempdir().unwrap();
    write_fixture(root.path());
    let app = root.path().join("app");
    let main = app.join("lib/main.dart");
    write(&main, MAIN);
    let mut server = Server::start(
        &app,
        &root.path().join("sdk"),
        root.path().join("stderr.log"),
        threads,
    );
    let uri = format!("file://{}", main.display());
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"processId":null,"rootUri":{:?},"capabilities":{{}}}}}}"#,
        format!("file://{}", app.display())
    ));
    server.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":{uri:?},"languageId":"dart","version":1,"text":{MAIN:?}}}}}}}"#
    ));

    let diagnostics = server.receive(Duration::from_secs(300), |message| {
        message.contains(r#""method":"textDocument/publishDiagnostics""#)
            && message.contains("lib/main.dart\"")
            && message.contains(RULE)
    });
    assert_eq!(
        diagnostics.matches(&format!(r#""code":"{RULE}""#)).count(),
        1,
        "{diagnostics}"
    );
    assert!(
        diagnostics.contains(r#""diagnostics":[{"range":{"start":{"line":3,"character":2}"#),
        "the call to the last exported function is reported: {diagnostics}"
    );

    server.send(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#);
    server.receive(Duration::from_secs(60), |message| {
        message.contains(r#""id":2"#) && message.contains(r#""result""#)
    });
    server.send(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#);
    let status = server.wait_for_exit(Duration::from_secs(60));
    if !status.success() {
        server.fail("the server exited unsuccessfully");
    }
}

#[test]
fn dependency_wave_resolves_with_one_rayon_thread() {
    publishes_resolved_dependency_diagnostics("1");
}

#[test]
fn dependency_wave_resolves_with_two_rayon_threads() {
    publishes_resolved_dependency_diagnostics("2");
}
