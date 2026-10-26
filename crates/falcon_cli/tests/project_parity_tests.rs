//! CLI and editor consumers must publish the same finalized project diagnostics.

use std::collections::BTreeMap;
use std::fs;

use falcon_cli::CheckOptions;
use falcon_diagnostics::Severity;
use falcon_lsp::LspState;
use tempfile::tempdir;

#[test]
fn cli_and_lsp_have_full_project_diagnostic_parity() {
    let temp = tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    fs::create_dir_all(root.join("lib/src")).unwrap();
    fs::write(root.join("pubspec.yaml"), "name: parity_fixture\n").unwrap();
    let config_path = root.join("falcon.json");
    fs::write(
        &config_path,
        r#"{
          "files": { "includes": ["**", "!lib/src/excluded.dart"] },
          "linter": {
            "rules": {
              "recommended": false,
              "suspicious": { "avoid-dynamic": "warn" }
            }
          },
          "cross-file": {
            "rules": {
              "recommended": false,
              "correctness": { "unused-files": "warn" }
            }
          },
          "overrides": [{
            "includes": ["lib/src/orphan.dart"],
            "linter": {
              "rules": { "suspicious": { "avoid-dynamic": "error" } }
            },
            "cross-file": {
              "rules": { "correctness": { "unused-files": "error" } }
            }
          }]
        }"#,
    )
    .unwrap();
    let files = [
        (
            "lib/main.dart",
            "import 'src/reachable.dart';\nimport 'src/suppressed.dart';\nvoid main() { use(); }\n",
        ),
        (
            "lib/src/reachable.dart",
            "// A reachable dependency retains the base warning severity.\ndynamic value = 1;\nvoid use() {}\n",
        ),
        (
            "lib/src/orphan.dart",
            "// Both file and project rules receive this path's override.\ndynamic orphan = 1;\n",
        ),
        (
            "lib/src/suppressed.dart",
            "dynamic legacy = 1; // falcon-ignore lint/suspicious/avoid-dynamic: legacy API\n",
        ),
        (
            "lib/src/excluded.dart",
            "// This would trigger both rules if admitted.\ndynamic excluded = 1;\n",
        ),
    ]
    .map(|(name, source)| {
        let path = root.join(name);
        fs::write(&path, source).unwrap();
        (path, source)
    });

    let cli = falcon_cli::collect_check_with_sdk_locator(
        &CheckOptions {
            paths: vec![root.clone()],
            config_path: Some(config_path.clone()),
            quiet: true,
            ..Default::default()
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(cli.total_files, 4, "the excluded Dart file is not admitted");
    assert_eq!(cli.exit_code, 1);

    let mut lsp = LspState::new_with_sdk_locator(Some(config_path), |_| None);
    let documents = files
        .iter()
        .map(|(path, source)| {
            let uri = format!("file://{}", path.display());
            // Also open the excluded file: admission must agree even when an
            // editor explicitly opens a document that the CLI walker excludes.
            lsp.open(&uri, (*source).to_owned(), Some(1));
            uri
        })
        .collect::<Vec<_>>();
    assert!(lsp.take_config_messages().is_empty());
    assert!(lsp.cross_file_rules_enabled());

    // Analyze only after every admitted document is open. Cross-file publication
    // replaces, rather than appends to, each document's per-file publication.
    let mut published = BTreeMap::new();
    for uri in &documents {
        published.insert(uri.clone(), lsp.analyze(uri));
    }
    for (uri, diagnostics) in lsp.cross_file_pass() {
        published.insert(uri, diagnostics);
    }
    assert!(published[&documents[4]].is_empty());
    assert!(published[&documents[3]].is_empty());
    // URI order is path order for this one local fixture; each publication has
    // already received the shared diagnostic sorting and position finalization.
    let editor_diagnostics = published.into_values().flatten().collect::<Vec<_>>();

    for diagnostics in [&cli.diagnostics, &editor_diagnostics] {
        assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
        for (file, rule, severity) in [
            ("lib/src/reachable.dart", "avoid-dynamic", Severity::Warning),
            ("lib/src/orphan.dart", "avoid-dynamic", Severity::Error),
            ("lib/src/orphan.dart", "unused-files", Severity::Error),
        ] {
            let path = root.join(file);
            let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.file_path == path.to_string_lossy() && diagnostic.rule == rule
                })
                .unwrap_or_else(|| panic!("missing {rule} in {file}: {diagnostics:#?}"));
            assert_eq!(diagnostic.severity, severity);
            assert!(!diagnostic.message.is_empty());
            assert!(diagnostic.line > 0 && diagnostic.col > 0);
            if rule == "unused-files" {
                assert!(diagnostic.message.contains("not reachable"));
                assert_eq!((diagnostic.line, diagnostic.col), (1, 1));
            } else {
                assert_eq!((diagnostic.line, diagnostic.col), (2, 1));
            }
        }
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic.file_path.ends_with("excluded.dart")
                && !diagnostic.file_path.ends_with("suppressed.dart")
        }));
    }
    // Compare every serialized diagnostic field, including messages, spans,
    // severity, resolved positions and fixes; no semantic fields are normalized.
    assert_eq!(
        serde_json::to_value(&cli.diagnostics).unwrap(),
        serde_json::to_value(&editor_diagnostics).unwrap()
    );
}

#[test]
fn cli_and_lsp_sort_syntax_errors_before_earlier_lints() {
    let temp = tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    let config_path = root.join("falcon.json");
    fs::write(
        &config_path,
        r#"{
          "linter": {
            "rules": {
              "recommended": false,
              "suspicious": { "avoid-dynamic": "warn" }
            }
          },
          "cross-file": { "enabled": false }
        }"#,
    )
    .unwrap();
    let path = root.join("main.dart");
    let source = "dynamic value = 1;\nvoid main() {\n  print(\"hello\"\n}\n";
    fs::write(&path, source).unwrap();
    let cli = falcon_cli::collect_check_with_sdk_locator(
        &CheckOptions {
            paths: vec![root],
            config_path: Some(config_path.clone()),
            quiet: true,
            ..Default::default()
        },
        |_| None,
    )
    .unwrap();
    let mut lsp = LspState::new_with_sdk_locator(Some(config_path), |_| None);
    let uri = format!("file://{}", path.display());
    lsp.open(&uri, source.to_owned(), Some(1));
    let editor_diagnostics = lsp.analyze(&uri);
    for diagnostics in [&cli.diagnostics, &editor_diagnostics] {
        let lint = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "avoid-dynamic")
            .expect("the early dynamic annotation must still produce a lint");
        let syntax = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "syntax-error")
            .expect("the later incomplete call must produce a syntax error");
        assert!(syntax.span.start > lint.span.start);
        assert_eq!(diagnostics[0].rule, "syntax-error");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.line > 0 && diagnostic.col > 0 })
        );
    }
    assert_eq!(
        serde_json::to_value(&cli.diagnostics).unwrap(),
        serde_json::to_value(&editor_diagnostics).unwrap()
    );
}
