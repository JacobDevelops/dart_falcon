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

/// Syntax-only rules analyze the open AST directly without parsing closed files
/// or constructing project-wide semantic indexes.
#[test]
fn non_resolving_rules_keep_the_single_document_fast_path() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-dynamic": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    fs::write(dir.path().join("closed.dart"), "void closed() {}\n").unwrap();
    let uri = format!("file://{}", dir.path().join("open.dart").display());

    let diagnostics = state.open(&uri, VIOLATING_SRC.to_string(), Some(1));
    assert!(diagnostics.iter().any(|d| d.rule == "avoid-dynamic"));
    assert_eq!(
        state.semantic_snapshot_build_count(),
        0,
        "syntax-only analysis must not walk and index the workspace"
    );

    assert!(state.change(&uri, CLEAN_SRC.to_string(), Some(2)));
    let affected = state.analyze_affected(&uri);
    assert_eq!(affected.len(), 1);
    assert_eq!(state.semantic_snapshot_build_count(), 0);
}

#[test]
fn changing_part_owner_refreshes_old_and_new_libraries() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let old_path = dir.path().join("old.dart");
    let new_path = dir.path().join("new.dart");
    let pivot_path = dir.path().join("pivot.dart");
    let old_uri = format!("file://{}", old_path.display());
    let new_uri = format!("file://{}", new_path.display());
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&old_path, "library old;\n").unwrap();
    fs::write(&new_path, "library new;\n").unwrap();
    fs::write(&pivot_path, "part of 'old.dart';\n").unwrap();

    state.open(&old_uri, "library old;\n".to_string(), Some(1));
    state.open(&new_uri, "library new;\n".to_string(), Some(1));
    state.open(&pivot_uri, "part of 'old.dart';\n".to_string(), Some(1));

    assert!(state.change(&pivot_uri, "part of 'new.dart';\n".to_string(), Some(2)));
    let affected = state.analyze_affected(&pivot_uri);
    let affected_uris = affected
        .iter()
        .map(|(uri, _)| uri.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        affected_uris,
        vec![new_uri.as_str(), old_uri.as_str(), pivot_uri.as_str()]
    );
}

#[test]
fn rebuilt_intermediate_graph_is_merged_into_pending_affected_documents() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let owners = ["a.dart", "b.dart", "c.dart"];
    let owner_uris = owners
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            fs::write(
                &path,
                format!("library {};\n", name.trim_end_matches(".dart")),
            )
            .unwrap();
            let uri = format!("file://{}", path.display());
            state.open(&uri, fs::read_to_string(path).unwrap(), Some(1));
            uri
        })
        .collect::<Vec<_>>();
    let pivot_path = dir.path().join("pivot.dart");
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&pivot_path, "part of 'a.dart';\n").unwrap();
    state.open(&pivot_uri, "part of 'a.dart';\n".to_string(), Some(1));

    assert!(state.change(&pivot_uri, "part of 'b.dart';\n".to_string(), Some(2)));
    state.analyze(&owner_uris[1]);
    assert!(state.change(&pivot_uri, "part of 'c.dart';\n".to_string(), Some(3)));

    let affected = state
        .analyze_affected(&pivot_uri)
        .into_iter()
        .map(|(uri, _)| uri)
        .collect::<Vec<_>>();
    assert_eq!(
        affected,
        vec![
            owner_uris[0].clone(),
            owner_uris[1].clone(),
            owner_uris[2].clone(),
            pivot_uri,
        ]
    );
}

#[test]
fn cross_file_publication_rebuild_is_merged_into_pending_affected_documents() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "rules": {
            "recommended": false,
            "correctness": { "unused-code": "warn" }
        } }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let owners = ["a.dart", "b.dart", "c.dart"];
    let owner_uris = owners
        .iter()
        .map(|name| {
            let stem = name.trim_end_matches(".dart");
            let source = format!("library {stem};\nvoid unused{stem}() {{}}\n");
            let path = dir.path().join(name);
            fs::write(&path, &source).unwrap();
            let uri = format!("file://{}", path.display());
            state.open(&uri, source, Some(1));
            uri
        })
        .collect::<Vec<_>>();
    let pivot_path = dir.path().join("pivot.dart");
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&pivot_path, "part of 'a.dart';\n").unwrap();
    state.open(&pivot_uri, "part of 'a.dart';\n".to_string(), Some(1));

    assert!(state.change(&pivot_uri, "part of 'b.dart';\n".to_string(), Some(2)));
    let builds_before = state.semantic_snapshot_build_count();
    assert!(
        !state.cross_file_pass().is_empty(),
        "the cross-file pass must publish and rebuild semantic state"
    );
    assert!(state.semantic_snapshot_build_count() > builds_before);
    assert!(state.change(&pivot_uri, "part of 'c.dart';\n".to_string(), Some(3)));

    let affected = state
        .analyze_affected(&pivot_uri)
        .into_iter()
        .map(|(uri, _)| uri)
        .collect::<Vec<_>>();
    assert_eq!(
        affected,
        vec![
            owner_uris[0].clone(),
            owner_uris[1].clone(),
            owner_uris[2].clone(),
            pivot_uri,
        ]
    );
}

#[test]
fn opening_buffer_with_new_part_owner_refreshes_disk_and_buffer_libraries() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let old_path = dir.path().join("old.dart");
    let new_path = dir.path().join("new.dart");
    let pivot_path = dir.path().join("pivot.dart");
    let old_uri = format!("file://{}", old_path.display());
    let new_uri = format!("file://{}", new_path.display());
    let pivot_uri = format!("file://{}", pivot_path.display());
    fs::write(&old_path, "library old;\n").unwrap();
    fs::write(&new_path, "library new;\n").unwrap();
    fs::write(&pivot_path, "part of 'old.dart';\n").unwrap();

    state.open(&old_uri, "library old;\n".to_string(), Some(1));
    state.open(&new_uri, "library new;\n".to_string(), Some(1));
    state.open(&pivot_uri, "part of 'new.dart';\n".to_string(), Some(1));
    let affected = state.analyze_affected(&pivot_uri);
    let affected_uris = affected
        .iter()
        .map(|(uri, _)| uri.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        affected_uris,
        vec![new_uri.as_str(), old_uri.as_str(), pivot_uri.as_str()]
    );
}

#[test]
fn multi_document_flush_analyzes_each_affected_document_once() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let api_path = dir.path().join("api.dart");
    let consumer_path = dir.path().join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let api = "int calculate() => 1;\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, api).unwrap();
    fs::write(&consumer_path, consumer).unwrap();
    state.open(&api_uri, api.to_string(), Some(1));
    state.open(&consumer_uri, consumer.to_string(), Some(1));

    assert!(state.change(&api_uri, "void calculate() {}\n".to_string(), Some(2)));
    assert!(state.change(&consumer_uri, format!("{consumer}\n"), Some(2)));
    let api_before = counts(&state, &api_uri).1;
    let consumer_before = counts(&state, &consumer_uri).1;

    let analyzed = state.analyze_affected_many(&[api_uri.clone(), consumer_uri.clone()]);
    assert_eq!(analyzed.len(), 2);
    assert_eq!(counts(&state, &api_uri).1, api_before + 1);
    assert_eq!(counts(&state, &consumer_uri).1, consumer_before + 1);
}

#[test]
fn affected_documents_share_one_invalidated_semantic_snapshot() {
    let config = r#"{
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    fs::write(dir.path().join("pubspec.yaml"), "name: workspace\n").unwrap();
    let api_path = dir.path().join("api.dart");
    let consumer_path = dir.path().join("consumer.dart");
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());
    let api = "int calculate() => 1;\n";
    let consumer = "import 'api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, api).unwrap();
    fs::write(&consumer_path, consumer).unwrap();

    state.open(&api_uri, api.to_string(), Some(1));
    state.open(&consumer_uri, consumer.to_string(), Some(1));
    assert_eq!(state.semantic_snapshot_build_count(), 2);
    assert_eq!(state.semantic_topology_build_count(), 2);

    assert!(state.change(&api_uri, "String calculate() => '';\n".to_string(), Some(2)));
    assert_eq!(
        state.semantic_snapshot_build_count(),
        2,
        "the first pending change retains the published semantic graph"
    );
    assert_eq!(
        state.semantic_topology_build_count(),
        2,
        "old-graph discovery must query the topology cached in the published snapshot"
    );
    assert!(state.change(&api_uri, "void calculate() {}\n".to_string(), Some(3)));
    assert_eq!(
        state.semantic_snapshot_build_count(),
        2,
        "repeated changes before the debounce flush must not rebuild the workspace"
    );
    assert_eq!(
        state.semantic_topology_build_count(),
        2,
        "repeated changes must not recompute intermediate dependency topologies"
    );
    let affected = state.analyze_affected(&api_uri);
    assert_eq!(
        affected.len(),
        2,
        "both the declaration and importer refresh"
    );
    assert_eq!(
        state.semantic_snapshot_build_count(),
        3,
        "dependency discovery and both analyses must share one rebuilt snapshot"
    );
    assert_eq!(
        state.semantic_topology_build_count(),
        3,
        "the final snapshot must build one topology shared by discovery and analysis"
    );

    state.save(&api_uri, Some("void calculate() {}\n".to_string()));
    assert_eq!(
        state.semantic_snapshot_build_count(),
        3,
        "an unchanged save reuses the current semantic snapshot"
    );

    state.close(&api_uri);
    state.analyze(&consumer_uri);
    assert_eq!(
        state.semantic_snapshot_build_count(),
        4,
        "closing a buffer rebuilds against its on-disk copy"
    );

    fs::write(dir.path().join("pubspec.yaml"), "name: renamed\n").unwrap();
    state.reload_config();
    assert_eq!(
        state.semantic_snapshot_build_count(),
        5,
        "watched config/package changes invalidate semantic package identities"
    );
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
fn current_package_import_resolves_and_invalidates_importer() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    fs::write(dir.path().join("pubspec.yaml"), "name: workspace\n").unwrap();
    let lib = dir.path().join("lib");
    fs::create_dir(&lib).unwrap();
    let api_path = lib.join("api.dart");
    let consumer_path = lib.join("consumer.dart");
    let api = "int calculate() => 1;\n";
    let consumer = "import 'package:workspace/api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, api).unwrap();
    fs::write(&consumer_path, consumer).unwrap();
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());

    state.open(&api_uri, api.to_string(), Some(1));
    let diagnostics = state.open(&consumer_uri, consumer.to_string(), Some(1));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values"),
        "the current package import must bind its workspace declaration: {diagnostics:?}"
    );

    let affected = state.analyze_affected(&api_uri);
    assert!(
        affected.iter().any(|(uri, _)| uri == &consumer_uri),
        "changing a current-package library must invalidate its importer: {affected:?}"
    );
}

#[test]
fn workspace_package_import_resolves_and_refreshes_semantic_diagnostics() {
    let config = r#"{
        "linter": { "rules": {
            "suspicious": { "avoid-ignoring-return-values": "warn" }
        } },
        "cross-file": { "enabled": false }
    }"#;
    let (mut state, dir) = state_with_config(config);
    let package_a = dir.path().join("packages/a");
    let package_b = dir.path().join("packages/b");
    fs::create_dir_all(package_a.join("lib")).unwrap();
    fs::create_dir_all(package_b.join("lib")).unwrap();
    fs::write(package_a.join("pubspec.yaml"), "name: a\n").unwrap();
    fs::write(package_b.join("pubspec.yaml"), "name: b\n").unwrap();
    let api_path = package_b.join("lib/api.dart");
    let consumer_path = package_a.join("lib/consumer.dart");
    let returning = "int calculate() => 1;\n";
    let returning_void = "void calculate() {}\n";
    let consumer = "import 'package:b/api.dart';\nvoid check() { calculate(); }\n";
    fs::write(&api_path, returning).unwrap();
    fs::write(&consumer_path, consumer).unwrap();
    let api_uri = format!("file://{}", api_path.display());
    let consumer_uri = format!("file://{}", consumer_path.display());

    state.open(&api_uri, returning.to_string(), Some(1));
    let initial = state.open(&consumer_uri, consumer.to_string(), Some(1));
    assert!(
        initial
            .iter()
            .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values"),
        "package:b must provide semantic return-type facts to package a: {initial:?}"
    );

    assert!(state.change(&api_uri, returning_void.to_string(), Some(2)));
    let affected = state.analyze_affected(&api_uri);
    let (_, diagnostics) = affected
        .iter()
        .find(|(uri, _)| uri == &consumer_uri)
        .expect("changing package b must reanalyze its package a importer");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != "avoid-ignoring-return-values"),
        "a void return in package b must clear package a's diagnostic: {diagnostics:?}"
    );

    assert!(state.change(&api_uri, returning.to_string(), Some(3)));
    let affected = state.analyze_affected(&api_uri);
    let (_, diagnostics) = affected
        .iter()
        .find(|(uri, _)| uri == &consumer_uri)
        .expect("changing package b again must reanalyze its package a importer");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values"),
        "restoring package b's value return must restore package a's diagnostic: {diagnostics:?}"
    );
}

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
