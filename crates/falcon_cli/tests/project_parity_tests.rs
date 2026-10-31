//! CLI and editor consumers must publish the same finalized project diagnostics.

use std::collections::BTreeMap;
use std::fs;

use falcon_cli::CheckOptions;
use falcon_diagnostics::Severity;
use falcon_lsp::LspState;
use tempfile::tempdir;

#[test]
fn optional_manifest_failures_recover_to_fresh_secure_url_diagnostics() {
    for broken in [b"homepage: [".as_slice(), &[0xff], b"oversized"] {
        let temp = tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let manifest = root.join("pubspec.yaml");
        let dart = root.join("main.dart");
        fs::write(&dart, "void main() {}\n").unwrap();
        let config = root.join("falcon.json");
        fs::write(&config, r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"security":{"secure-pubspec-urls":"warn"}}}}"#).unwrap();
        fs::write(&manifest, "name: example\nhomepage: http://example.com\n").unwrap();
        let options = CheckOptions {
            paths: vec![root],
            config_path: Some(config.clone()),
            quiet: true,
            ..Default::default()
        };
        let control = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
        assert_eq!(control.diagnostics.len(), 1);
        assert_eq!(control.diagnostics[0].rule, "secure-pubspec-urls");
        fs::write(&manifest, broken).unwrap();
        if broken == b"oversized" {
            fs::File::options()
                .write(true)
                .open(&manifest)
                .unwrap()
                .set_len(falcon_config::MAX_METADATA_BYTES + 1)
                .unwrap();
        }
        let mut lsp = LspState::new_with_sdk_locator(Some(config), |_| None);
        let uri = format!("file://{}", dart.display());
        let manifest_uri = format!("file://{}", manifest.display());
        lsp.open(&uri, "void main() {}\n".into(), Some(1));
        let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
        assert!(cli.diagnostics.is_empty(), "{:?}", cli.diagnostics);
        assert!(
            lsp.cross_file_pass()
                .iter()
                .all(|(_, diagnostics)| diagnostics.is_empty())
        );
        let warnings = lsp.take_config_messages();
        assert_eq!(
            warnings
                .iter()
                .filter(|warning| warning.message.contains(&*manifest.to_string_lossy()))
                .count(),
            1,
            "{warnings:?}"
        );
        fs::write(&manifest, "name: example\nhomepage: http://example.com\n").unwrap();
        lsp.watched_files_changed(std::slice::from_ref(&manifest));
        let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
        lsp.open(
            &manifest_uri,
            fs::read_to_string(&manifest).unwrap(),
            Some(1),
        );
        let published = lsp.cross_file_pass();
        let diagnostics = &published
            .iter()
            .find(|(uri, _)| uri == &manifest_uri)
            .unwrap()
            .1;
        assert_eq!(
            serde_json::to_value(diagnostics).unwrap(),
            serde_json::to_value(&cli.diagnostics).unwrap()
        );
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule, "secure-pubspec-urls");
        assert_eq!(diagnostic.file_path, manifest.to_string_lossy());
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(
            diagnostic.message,
            "The `http` protocol shouldn't be used because it isn't secure."
        );
        assert_eq!((diagnostic.line, diagnostic.col), (2, 11));
        assert_eq!((diagnostic.span.start, diagnostic.span.end), (24, 42));
        assert!(lsp.change(
            &manifest_uri,
            "name: example\nhomepage: https://example.com\n".into(),
            Some(2)
        ));
        let unsaved = lsp.cross_file_pass();
        assert!(unsaved.iter().any(|(_, diagnostics)| {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "secure-pubspec-urls")
        }));
        fs::write(&manifest, "name: example\nhomepage: https://example.com\n").unwrap();
        lsp.watched_files_changed(std::slice::from_ref(&manifest));
        assert!(
            lsp.cross_file_pass()
                .iter()
                .all(|(_, diagnostics)| diagnostics.is_empty())
        );
    }
}

#[test]
fn dependency_manifest_consumers_recover_exact_diagnostics_and_warning_episodes() {
    for broken in [b"dependencies: [".as_slice(), &[0xff], b"oversized"] {
        let temp = tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let manifest = root.join("pubspec.yaml");
        let dart = root.join("main.dart");
        let source = "import 'package:missing/missing.dart';\nvoid main() {}\n";
        let valid = "name: BadName\ndependencies:\n  zebra: any\n  apple: any\n";
        fs::write(&dart, source).unwrap();
        fs::write(&manifest, valid).unwrap();
        let config = root.join("falcon.json");
        fs::write(&config, r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"style":{"package-names":"warn"},"correctness":{"sort-pub-dependencies":"warn","undeclared-dependencies":"warn"}}}}"#).unwrap();
        let options = CheckOptions {
            paths: vec![root],
            config_path: Some(config.clone()),
            quiet: true,
            ..Default::default()
        };
        let control = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
        assert_eq!(control.diagnostics.len(), 3, "{:?}", control.diagnostics);
        for rule in [
            "sort-pub-dependencies",
            "package-names",
            "undeclared-dependencies",
        ] {
            assert!(
                control
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == rule)
            );
        }
        let expected = serde_json::to_value(&control.diagnostics).unwrap();
        let mut lsp = LspState::new_with_sdk_locator(Some(config), |_| None);
        lsp.open(
            &format!("file://{}", dart.display()),
            source.into(),
            Some(1),
        );
        lsp.open(
            &format!("file://{}", manifest.display()),
            valid.into(),
            Some(1),
        );
        let collect = |lsp: &mut LspState| {
            let mut diagnostics = lsp
                .cross_file_pass()
                .into_iter()
                .flat_map(|(_, diagnostics)| diagnostics)
                .collect::<Vec<_>>();
            diagnostics.sort_by(|left, right| {
                left.file_path
                    .cmp(&right.file_path)
                    .then(left.span.start.cmp(&right.span.start))
                    .then(left.rule.cmp(right.rule))
            });
            diagnostics
        };
        assert_eq!(serde_json::to_value(collect(&mut lsp)).unwrap(), expected);
        assert_eq!(lsp.take_config_messages(), Vec::new());
        for _ in 0..2 {
            fs::write(&manifest, broken).unwrap();
            if broken == b"oversized" {
                fs::OpenOptions::new()
                    .write(true)
                    .open(&manifest)
                    .unwrap()
                    .set_len(falcon_config::MAX_METADATA_BYTES + 1)
                    .unwrap();
            }
            lsp.watched_files_changed(std::slice::from_ref(&manifest));
            let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
            assert!(cli.diagnostics.is_empty(), "{:?}", cli.diagnostics);
            assert!(collect(&mut lsp).is_empty());
            let warnings = lsp.take_config_messages();
            assert_eq!(
                warnings
                    .iter()
                    .filter(|warning| warning.message.contains(&*manifest.to_string_lossy()))
                    .count(),
                1,
                "{warnings:?}"
            );
            assert!(collect(&mut lsp).is_empty());
            assert_eq!(lsp.take_config_messages(), Vec::new());
            fs::write(&manifest, valid).unwrap();
            lsp.watched_files_changed(std::slice::from_ref(&manifest));
            let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
            assert_eq!(serde_json::to_value(&cli.diagnostics).unwrap(), expected);
            assert_eq!(serde_json::to_value(collect(&mut lsp)).unwrap(), expected);
            assert_eq!(lsp.take_config_messages(), Vec::new());
        }
    }
}

#[cfg(unix)]
#[test]
fn explicit_metadata_failure_exits_nonzero_and_names_the_input() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, "{}").unwrap();
    let path = temp.path().join("loop.dart");
    std::os::unix::fs::symlink(&path, &path).unwrap();
    for input in [path.clone(), temp.path().to_path_buf()] {
        let options = CheckOptions {
            paths: vec![input],
            config_path: Some(config.clone()),
            quiet: true,
            ..Default::default()
        };
        let error = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
        assert!(error.contains("failed to access input"), "{error}");
        assert!(error.contains(&*path.to_string_lossy()), "{error}");
        assert!(
            error.contains(&path.metadata().unwrap_err().to_string()),
            "{error}"
        );
        assert_ne!(
            falcon_cli::run_check_with_sdk_locator(&options, |_| None),
            0
        );
    }
}

#[cfg(unix)]
#[test]
fn unreadable_directory_blocks_cli_and_editor_results_and_recovers() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    let lib = root.join("lib");
    let hidden = lib.join("hidden");
    fs::create_dir_all(&hidden).unwrap();
    fs::create_dir(lib.join("src")).unwrap();
    fs::write(root.join("pubspec.yaml"), "name: traversal_fixture\n").unwrap();
    let config = root.join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
    ).unwrap();
    fs::write(
        lib.join("main.dart"),
        "import 'hidden/bridge.dart';\nvoid main() {}\n",
    )
    .unwrap();
    let bridge = hidden.join("bridge.dart");
    fs::write(&bridge, "// no export\n").unwrap();
    let orphan = lib.join("src/orphan.dart");
    fs::write(&orphan, "class Orphan {}\n").unwrap();
    let options = CheckOptions {
        paths: vec![root],
        config_path: Some(config.clone()),
        quiet: true,
        ..Default::default()
    };
    let uri = format!("file://{}", orphan.display());
    let mut lsp = LspState::new_with_sdk_locator(Some(config), |_| None);
    lsp.open(&uri, "class Orphan {}\n".into(), Some(1));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(cli.diagnostics.len(), 1);
    assert_eq!(cli.diagnostics[0].rule, "unused-files");
    assert_eq!(cli.diagnostics[0].file_path, orphan.to_string_lossy());
    let published = lsp.cross_file_pass();
    assert_eq!(
        serde_json::to_value(&published.iter().find(|(key, _)| key == &uri).unwrap().1).unwrap(),
        serde_json::to_value(&cli.diagnostics).unwrap()
    );
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o0)).unwrap();
    if fs::read_dir(&hidden).is_ok() {
        fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!(
            "skipped traversal parity permission assertion: this user bypasses directory permissions"
        );
        return;
    }
    let error = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
    assert!(error.contains(&*hidden.to_string_lossy()), "{error}");
    assert_ne!(
        falcon_cli::run_check_with_sdk_locator(&options, |_| None),
        0
    );
    lsp.watched_files_changed(std::slice::from_ref(&hidden));
    let published = lsp.cross_file_pass();
    assert!(
        published
            .iter()
            .find(|(key, _)| key == &uri)
            .unwrap()
            .1
            .is_empty()
    );
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&bridge, "export '../src/orphan.dart';\n").unwrap();
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert!(cli.diagnostics.is_empty());
    assert_eq!(cli.exit_code, 0);
    assert!(
        lsp.cross_file_pass()
            .iter()
            .all(|(_, diagnostics)| diagnostics.is_empty())
    );
    fs::write(&bridge, "// export removed again\n").unwrap();
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    let published = lsp.cross_file_pass();
    let editor = &published.iter().find(|(key, _)| key == &uri).unwrap().1;
    assert_eq!(
        serde_json::to_value(editor).unwrap(),
        serde_json::to_value(&cli.diagnostics).unwrap()
    );
    assert_eq!(editor.len(), 1);
    assert_eq!(editor[0].rule, "unused-files");
    assert_eq!(editor[0].file_path, orphan.to_string_lossy());
    assert_eq!(editor[0].severity, Severity::Warning);
    assert!(editor[0].message.contains("not reachable"));
    assert_eq!((editor[0].line, editor[0].col), (1, 1));
}

#[cfg(unix)]
#[test]
fn unreadable_directories_fail_only_when_included_descendants_are_possible() {
    use std::os::unix::fs::PermissionsExt;
    for (includes, hidden_relative, member, complete) in [
        (vec!["lib/**"], "outside", None, true),
        (vec!["lib/**"], "lib/hidden", None, false),
        (vec![], "outside", None, false),
        (
            vec!["lib/**"],
            "packages/child",
            Some("packages/child"),
            false,
        ),
        (vec!["lib/**"], "packages/child", Some("packages/*"), false),
        (vec!["lib/**"], "packages", Some("packages/child"), false),
    ] {
        let temp = tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let lib = root.join("lib");
        let hidden = root.join(hidden_relative);
        fs::create_dir_all(lib.join("src")).unwrap();
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("source.dart"), "class Hidden {}").unwrap();
        fs::write(root.join("pubspec.yaml"), "name: scope_fixture\n").unwrap();
        if let Some(member) = member {
            fs::write(
                root.join("pubspec.yaml"),
                format!("name: scope_fixture\nworkspace:\n  - '{member}'\n"),
            )
            .unwrap();
            let child = root.join("packages/child");
            fs::create_dir_all(&child).unwrap();
            fs::write(
                child.join("pubspec.yaml"),
                "name: child\nresolution: workspace\n",
            )
            .unwrap();
        }
        fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();
        let orphan = lib.join("src/orphan.dart");
        fs::write(&orphan, "class Orphan {}\n").unwrap();
        let config = root.join("falcon.json");
        fs::write(
            &config,
            serde_json::json!({
                "files": {"includes": includes},
                "linter": {"rules": {"recommended": false}},
                "cross-file": {"rules": {
                    "recommended": false, "correctness": {"unused-files": "warn"}
                }}
            })
            .to_string(),
        )
        .unwrap();
        let mut lsp = LspState::new_with_sdk_locator(Some(config.clone()), |_| None);
        let uri = format!("file://{}", orphan.display());
        lsp.open(&uri, "class Orphan {}\n".into(), Some(1));
        let baseline = lsp.cross_file_pass();
        assert_eq!(
            baseline
                .iter()
                .find(|(key, _)| key == &uri)
                .unwrap()
                .1
                .len(),
            1
        );
        fs::set_permissions(&hidden, fs::Permissions::from_mode(0o0)).unwrap();
        if fs::read_dir(&hidden).is_ok() {
            fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
            eprintln!(
                "skipped include-scope permission assertion: this user bypasses directory permissions"
            );
            continue;
        }
        let options = CheckOptions {
            paths: vec![root],
            config_path: Some(config.clone()),
            quiet: true,
            ..Default::default()
        };
        let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None);
        lsp.watched_files_changed(std::slice::from_ref(&hidden));
        let published = lsp.cross_file_pass();
        fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(cli.is_ok(), complete, "{includes:?}: {hidden_relative}");
        let diagnostics = &published.iter().find(|(key, _)| key == &uri).unwrap().1;
        assert_eq!(diagnostics.len(), usize::from(complete));
        if let Ok(cli) = cli {
            assert_eq!(cli.diagnostics.len(), 1);
            assert_eq!(diagnostics[0].rule, "unused-files");
        } else {
            assert!(cli.unwrap_err().contains(&*hidden.to_string_lossy()));
        }
    }
}

#[test]
fn required_source_failure_blocks_partial_projects_and_recovers() {
    let temp = tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    let lib = root.join("lib");
    fs::create_dir_all(lib.join("src")).unwrap();
    fs::write(root.join("pubspec.yaml"), "name: acquisition_fixture\n").unwrap();
    let config = root.join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
    )
    .unwrap();
    fs::write(
        lib.join("main.dart"),
        "import 'bridge.dart';\nvoid main() {}\n",
    )
    .unwrap();
    let bridge = lib.join("bridge.dart");
    let orphan = lib.join("src/orphan.dart");
    fs::write(&bridge, "export 'src/orphan.dart';\n").unwrap();
    fs::write(&orphan, "class Orphan {}\n").unwrap();
    let options = CheckOptions {
        paths: vec![root],
        config_path: Some(config.clone()),
        quiet: true,
        ..Default::default()
    };
    let uri = format!("file://{}", orphan.display());
    let mut lsp = LspState::new_with_sdk_locator(Some(config), |_| None);
    lsp.open(&uri, "class Orphan {}\n".into(), Some(1));
    assert!(
        falcon_cli::collect_check_with_sdk_locator(&options, |_| None)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    assert!(
        lsp.cross_file_pass()
            .iter()
            .all(|(_, diagnostics)| diagnostics.is_empty())
    );
    fs::write(&bridge, "// no export\n").unwrap();
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    let published = lsp.cross_file_pass();
    let (_, editor) = published.iter().find(|(key, _)| key == &uri).unwrap();
    for diagnostics in [&cli.diagnostics, editor] {
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule, "unused-files");
        assert_eq!(diagnostic.file_path, orphan.to_string_lossy());
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.message.contains("not reachable"));
        assert_eq!((diagnostic.line, diagnostic.col), (1, 1));
    }

    // Without bridge.dart the orphan appears unreachable; failed acquisition
    // must clear earlier project results instead of publishing that false claim.
    fs::write(&bridge, [0xff]).unwrap();
    let error = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap_err();
    assert!(error.contains(&*bridge.to_string_lossy()), "{error}");
    assert!(error.contains("UTF-8"), "{error}");
    assert_ne!(
        falcon_cli::run_check_with_sdk_locator(&options, |_| None),
        0
    );
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let published = lsp.cross_file_pass();
    let (_, diagnostics) = published.iter().find(|(key, _)| key == &uri).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");

    // Restoring the export must not resurrect the pre-failure orphan warning.
    fs::write(&bridge, "export 'src/orphan.dart';\n").unwrap();
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(cli.exit_code, 0);
    assert!(cli.diagnostics.is_empty(), "{:#?}", cli.diagnostics);
    let published = lsp.cross_file_pass();
    assert!(
        published
            .iter()
            .all(|(_, diagnostics)| diagnostics.is_empty()),
        "{published:#?}"
    );

    // A new absence warning proves recovery did not leave the LSP incomplete.
    fs::write(&bridge, "// no export\n").unwrap();
    lsp.watched_files_changed(std::slice::from_ref(&bridge));
    let cli = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(cli.exit_code, 0);
    let published = lsp.cross_file_pass();
    let (_, editor) = published.iter().find(|(key, _)| key == &uri).unwrap();
    for diagnostics in [&cli.diagnostics, editor] {
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule, "unused-files");
        assert_eq!(diagnostic.file_path, orphan.to_string_lossy());
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.message.contains("not reachable"));
        assert_eq!((diagnostic.line, diagnostic.col), (1, 1));
    }
}

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
    assert_eq!(lsp.take_config_messages(), Vec::new());
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
        assert_project_diagnostics(&root, diagnostics);
    }
    // Compare every serialized diagnostic field, including messages, spans,
    // severity, resolved positions and fixes; no semantic fields are normalized.
    assert_eq!(
        serde_json::to_value(&cli.diagnostics).unwrap(),
        serde_json::to_value(&editor_diagnostics).unwrap()
    );
}

fn assert_project_diagnostics(
    root: &std::path::Path,
    diagnostics: &[falcon_diagnostics::Diagnostic],
) {
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
        assert_ne!(diagnostic.message, "");
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

#[test]
fn cli_and_lsp_have_sdk_resolution_parity_for_unsaved_buffers() {
    fn fixture_sdk(
        config: Option<&falcon_analyze::PackageConfig>,
    ) -> Option<falcon_analyze::DartSdk> {
        falcon_analyze::DartSdk::load(&config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"))
    }

    let temp = tempdir().unwrap();
    let workspace = fs::canonicalize(temp.path()).unwrap();
    let root = workspace.join("app");
    let flutter = workspace.join("flutter");
    let sdk = flutter.join("bin/cache/dart-sdk");
    write_sdk_parity_fixture(&root, &flutter, &sdk);
    let config_path = root.join("falcon.json");
    fs::write(
        &config_path,
        r#"{
          "linter": {
            "rules": {
              "recommended": false,
              "correctness": { "discarded-futures": "warn" }
            }
          },
          "cross-file": { "enabled": false }
        }"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/pending.dart"),
        "import 'dart:async';\nFuture<void> pending() async {}\n",
    )
    .unwrap();
    let path = root.join("lib/main.dart");
    let source = "import 'pending.dart';\nvoid main() {\n  pending();\n}\n";
    fs::write(&path, source).unwrap();
    let cli = falcon_cli::collect_check_with_sdk_locator(
        &CheckOptions {
            paths: vec![root],
            config_path: Some(config_path.clone()),
            quiet: true,
            ..Default::default()
        },
        fixture_sdk,
    )
    .unwrap();
    assert_eq!(cli.total_files, 2);
    assert_eq!(cli.exit_code, 0);

    // CLI analyzed the edited snapshot. Leave a clean version on disk so LSP
    // must resolve the changed buffer, with its dependency still unopened.
    let saved = "import 'pending.dart';\nvoid main() {}\n";
    fs::write(&path, saved).unwrap();
    let mut lsp = LspState::new_with_sdk_locator(Some(config_path), fixture_sdk);
    let uri = format!("file://{}", path.display());
    lsp.open(&uri, saved.to_owned(), Some(1));
    assert!(lsp.analyze(&uri).is_empty());
    assert!(lsp.change(&uri, source.to_owned(), Some(2)));
    let editor_diagnostics = lsp.analyze(&uri);
    assert_eq!(lsp.take_config_messages(), Vec::new());
    assert_eq!(fs::read_to_string(&path).unwrap(), saved);

    for diagnostics in [&cli.diagnostics, &editor_diagnostics] {
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule, "discarded-futures");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.file_path, path.to_string_lossy());
        assert_eq!(
            diagnostic.message,
            "Avoid invoking an asynchronous function in a non-`async` function."
        );
        assert_eq!((diagnostic.line, diagnostic.col), (3, 3));
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            "pending"
        );
    }
    assert_eq!(
        serde_json::to_value(&cli.diagnostics).unwrap(),
        serde_json::to_value(&editor_diagnostics).unwrap()
    );
}

fn write_sdk_parity_fixture(
    root: &std::path::Path,
    flutter: &std::path::Path,
    sdk: &std::path::Path,
) {
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::create_dir_all(root.join(".dart_tool")).unwrap();
    fs::write(
        root.join(".dart_tool/package_config.json"),
        format!(
            r#"{{"configVersion":2,"flutterRoot":"file://{}","packages":[
                {{"name":"sdk_parity_fixture","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"}}
            ]}}"#,
            flutter.display(),
        ),
    )
    .unwrap();
    fs::create_dir_all(sdk.join("lib/core")).unwrap();
    fs::create_dir_all(sdk.join("lib/async")).unwrap();
    // Follow the external-library fixtures: resolve real parsed SDK declarations
    // through libraries.json, without depending on a locally installed SDK.
    fs::write(
        sdk.join("lib/libraries.json"),
        r#"{"vm":{"libraries":{
            "core":{"uri":"core/core.dart"},
            "async":{"uri":"async/async.dart"}
        }}}"#,
    )
    .unwrap();
    fs::write(
        sdk.join("lib/core/core.dart"),
        "library dart.core;\nclass Object {}\n",
    )
    .unwrap();
    fs::write(
        sdk.join("lib/async/async.dart"),
        "library dart.async;\nabstract class Future<T> {}\n",
    )
    .unwrap();
    fs::write(root.join("pubspec.yaml"), "name: sdk_parity_fixture\n").unwrap();
}
