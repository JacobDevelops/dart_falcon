//! Incremental analysis & caching tests (M5.2), against `LspState` directly.
//!
//! These prove the two cache-invalidation axes from LSP_CACHING_DESIGN.md:
//! text changes re-parse only the changed document; config changes rebuild
//! the rule set and re-analyze cached ASTs without any re-parse.

use std::fs;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use falcon_lsp::LspState;

const VIOLATING_SRC: &str = "void f() {\n  dynamic x = 1;\n  print(x);\n}\n";
const CLEAN_SRC: &str = "void g() {\n  final int y = 2;\n  print(y);\n}\n";

const URI_A: &str = "file:///test/a.dart";
const URI_B: &str = "file:///test/b.dart";

/// State with a hermetic config file (defaults unless `json` given).
fn state_with_config(json: &str) -> (LspState, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("falcon.json");
    fs::write(&path, json).unwrap();
    (LspState::new(Some(path)), dir)
}

fn counts(state: &LspState, uri: &str) -> (u64, u64) {
    let doc = state.document(uri).expect("document open");
    (doc.parse_count, doc.analyze_count)
}

/// Changing one file re-parses and re-analyzes only that file; other open
/// documents are untouched.
#[test]
fn only_changed_file_is_reanalyzed() {
    let (mut state, _dir) = state_with_config("{}");
    state.open(URI_A, VIOLATING_SRC.to_string(), Some(1));
    state.open(URI_B, CLEAN_SRC.to_string(), Some(1));
    assert_eq!(counts(&state, URI_A), (1, 1));
    assert_eq!(counts(&state, URI_B), (1, 1));

    assert!(state.change(URI_A, CLEAN_SRC.to_string(), Some(2)));
    state.analyze(URI_A); // server loop flush

    assert_eq!(counts(&state, URI_A), (2, 2), "changed file re-analyzed");
    assert_eq!(counts(&state, URI_B), (1, 1), "other file untouched");
}

/// An inline `// falcon-ignore` comment suppresses the diagnostic through the
/// LSP analyze path, just as it does in the CLI pipeline.
#[test]
fn inline_ignore_suppresses_in_lsp() {
    let (mut state, _dir) = state_with_config("{}");
    let suppressed = "void f() {\n  dynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic: legacy\n  print(x);\n}\n";
    let diagnostics = state.open(URI_A, suppressed.to_string(), Some(1));
    assert!(
        diagnostics.iter().all(|d| d.rule != "avoid-dynamic"),
        "inline falcon-ignore must suppress avoid-dynamic in the LSP path"
    );
}

/// A malformed `// falcon-ignore` (no reason) does not suppress and surfaces a
/// `malformed-suppression` diagnostic through the LSP path.
#[test]
fn malformed_suppression_reported_in_lsp() {
    let (mut state, _dir) = state_with_config("{}");
    let src = "void f() {\n  dynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic\n  print(x);\n}\n";
    let diagnostics = state.open(URI_A, src.to_string(), Some(1));
    assert!(
        diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "a reasonless falcon-ignore must not suppress"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.rule == "malformed-suppression"),
        "a reasonless falcon-ignore must report malformed-suppression"
    );
}

/// Config reload re-analyzes every open document against its cached AST —
/// rule set changes take effect with zero re-parses. This is the
/// stale-AST-with-new-config guard from the design doc.
#[test]
fn config_reload_reuses_cached_asts() {
    let (mut state, dir) = state_with_config("{}");
    let diagnostics = state.open(URI_A, VIOLATING_SRC.to_string(), Some(1));
    assert!(
        diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "violation fires under default config"
    );
    state.open(URI_B, CLEAN_SRC.to_string(), Some(1));

    fs::write(
        dir.path().join("falcon.json"),
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }"#,
    )
    .unwrap();
    let results = state.reload_config();

    assert_eq!(results.len(), 2, "every open document re-analyzed");
    let (_, diags_a) = results.iter().find(|(uri, _)| uri == URI_A).unwrap();
    assert!(
        diags_a.iter().all(|d| d.rule != "avoid-dynamic"),
        "new config must apply to cached AST: {diags_a:?}"
    );
    assert_eq!(counts(&state, URI_A), (1, 2), "re-analyzed, NOT re-parsed");
    assert_eq!(counts(&state, URI_B), (1, 2), "re-analyzed, NOT re-parsed");
}

/// didSave with identical text must not re-parse (text comparison guard).
#[test]
fn save_with_unchanged_text_does_not_reparse() {
    let (mut state, _dir) = state_with_config("{}");
    state.open(URI_A, VIOLATING_SRC.to_string(), Some(1));

    state.save(URI_A, Some(VIOLATING_SRC.to_string()));
    assert_eq!(counts(&state, URI_A), (1, 2), "analyze yes, re-parse no");

    state.save(URI_A, Some(CLEAN_SRC.to_string()));
    assert_eq!(counts(&state, URI_A), (2, 3), "differing text re-parses");
}

/// Operations on closed documents are safe no-ops.
#[test]
fn closed_document_operations_are_noops() {
    let (mut state, _dir) = state_with_config("{}");
    state.open(URI_A, CLEAN_SRC.to_string(), Some(1));
    state.close(URI_A);

    assert!(state.document(URI_A).is_none());
    assert!(!state.change(URI_A, VIOLATING_SRC.to_string(), Some(2)));
    assert!(state.analyze(URI_A).is_empty());
    assert!(state.open_uris().is_empty());
}

/// M5.4 gate: single-file incremental re-analyze (change + analyze) must
/// complete in <100ms. Uses a generated ~600-line file so the bound is
/// exercised on a realistically large document.
#[test]
fn incremental_reanalyze_under_100ms() {
    let mut source = String::from("class Generated {\n");
    for i in 0..200 {
        source.push_str(&format!(
            "  int method{i}(int value) {{\n    final int result{i} = value + {i};\n    return result{i};\n  }}\n"
        ));
    }
    source.push_str("}\n");

    let (mut state, _dir) = state_with_config("{}");
    state.open(URI_A, source.clone(), Some(1));

    source.push_str("\nvoid extra() {\n  dynamic z = 1;\n  print(z);\n}\n");
    let start = Instant::now();
    state.change(URI_A, source, Some(2));
    let diagnostics = state.analyze(URI_A);
    let elapsed = start.elapsed();

    assert!(
        diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "sanity: edit introduced a violation"
    );
    assert!(
        elapsed < Duration::from_millis(100),
        "incremental re-analyze took {elapsed:?} (gate: <100ms)"
    );
}
