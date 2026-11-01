use std::sync::Arc;

use falcon_analyze::{CrossFileRuleAnalysis, ParsedFile, ProjectFile};
use falcon_config::FalconConfig;

use super::depend_on_referenced_packages::DependOnReferencedPackages;
use super::undeclared_dependencies::UndeclaredDependencies;

#[test]
fn undeclared_rule_owns_overlap_but_not_executable_dev_dependencies() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("pubspec.yaml"),
        "name: app\ndev_dependencies:\n  dev: any\n",
    )
    .unwrap();
    let files: Vec<_> = [
        (
            "lib/api.dart",
            "import 'package:missing/api.dart';\nimport 'package:dev/api.dart';",
        ),
        ("bin/main.dart", "import 'package:dev/api.dart';"),
    ]
    .into_iter()
    .map(|(path, source)| {
        ProjectFile::new(root.path().join(path), Arc::new(ParsedFile::parse(source)))
            .expect("parsed fixture must be accepted by the project")
    })
    .collect();
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    let config: FalconConfig = serde_json::from_value(serde_json::json!({
        "cross-file": { "rules": { "correctness": {
            "undeclared-dependencies": "warn",
            "depend-on-referenced-packages": "warn"
        } } }
    }))
    .unwrap();
    assert_eq!(
        UndeclaredDependencies
            .analyze_project(
                "undeclared-dependencies",
                &environment,
                &files,
                &config,
                None,
            )
            .len(),
        2
    );
    let legacy = DependOnReferencedPackages.analyze_project(
        "depend-on-referenced-packages",
        &environment,
        &files,
        &config,
        None,
    );
    assert_eq!(legacy.len(), 1);
    assert!(legacy[0].file_path.ends_with("bin/main.dart"));
    let standalone: FalconConfig = serde_json::from_value(serde_json::json!({
        "cross-file": { "rules": { "correctness": {
            "undeclared-dependencies": "off"
        } } }
    }))
    .unwrap();
    assert_eq!(
        DependOnReferencedPackages
            .analyze_project(
                "depend-on-referenced-packages",
                &environment,
                &files,
                &standalone,
                None,
            )
            .len(),
        3
    );
}

#[test]
fn manifest_consumers_use_captured_facts_without_reopening_the_manifest() {
    use falcon_analyze::CrossFileRule;

    let root = tempfile::tempdir().unwrap();
    let manifest = root.path().join("pubspec.yaml");
    std::fs::write(
        &manifest,
        "name: BadName\ndependencies:\n  zebra: any\n  apple: any\ndev_dependencies:\n  tool: any\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join(".dart_tool")).unwrap();
    std::fs::create_dir(root.path().join("tool")).unwrap();
    std::fs::write(
        root.path().join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"tool","rootUri":"../tool","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    let files = [ProjectFile::new(
        root.path().join("main.dart"),
        Arc::new(ParsedFile::parse("import 'package:missing/api.dart';")),
    )
    .unwrap()];
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    let config: FalconConfig = serde_json::from_value(serde_json::json!({
        "cross-file": { "rules": { "correctness": {
            "undeclared-dependencies": "off",
            "banned-imports": {"level": "warn", "options": {
                "rules": [{"files": ["*.dart"], "deny": ["package:missing/*"],
                    "message": "Keep package boundaries."}]
            }}
        }}}
    }))
    .unwrap();
    let rules: [&dyn CrossFileRule; 7] = [
        &super::sort_pub_dependencies::SortPubDependencies,
        &super::package_names::PackageNames,
        &UndeclaredDependencies,
        &DependOnReferencedPackages,
        &super::unused_dependencies::UnusedDependencies,
        &super::unused_dev_dependencies::UnusedDevDependencies,
        &super::banned_imports::BannedImports,
    ];
    let control: Vec<_> = rules
        .iter()
        .map(|rule| rule.prepare_project(&environment, &files, &config, None))
        .collect();
    assert_eq!(
        control.iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 1, 1, 1, 2, 1, 1]
    );
    std::fs::remove_file(&manifest).unwrap();
    for (rule, expected) in rules.iter().zip(&control) {
        assert_eq!(
            serde_json::to_value(rule.prepare_project(&environment, &files, &config, None))
                .unwrap(),
            serde_json::to_value(expected).unwrap(),
        );
    }
    let missing = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    for rule in rules {
        assert!(
            rule.prepare_project(&missing, &files, &config, None)
                .is_empty()
        );
    }
    assert_eq!(missing.take_warnings(), Vec::<String>::new());
    for broken in [b"dependencies: [".as_slice(), &[0xff]] {
        std::fs::write(&manifest, broken).unwrap();
        let invalid = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        for rule in rules.iter().take(6) {
            assert!(
                rule.prepare_project(&invalid, &files, &config, None)
                    .is_empty()
            );
        }
        assert_eq!(
            serde_json::to_value(rules[6].prepare_project(&invalid, &files, &config, None))
                .unwrap(),
            serde_json::to_value(&control[6]).unwrap(),
        );
        let warnings = invalid.take_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains(&*manifest.to_string_lossy()));
        assert_eq!(invalid.take_warnings(), Vec::<String>::new());
    }
}

#[test]
fn malformed_manifest_fallback_name_preserves_own_package_path_boundaries() {
    use falcon_analyze::CrossFileRule;

    let root = tempfile::tempdir().unwrap();
    let manifest = root.path().join("pubspec.yaml");
    let files = [ProjectFile::new(
        root.path().join("lib/main.dart"),
        Arc::new(ParsedFile::parse("import 'package:app/private.dart';")),
    )
    .unwrap()];
    let config: FalconConfig = serde_json::from_value(serde_json::json!({
        "cross-file": {"rules": {"correctness": {
            "banned-imports": {"level": "warn", "options": {
                "rules": [{"files": ["lib/**"], "deny": ["lib/private.dart"],
                    "message": "Keep package boundaries."}]
            }}
        }}}
    }))
    .unwrap();
    let rule = super::banned_imports::BannedImports;
    std::fs::write(&manifest, "name: app\n").unwrap();
    let valid = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    let control = rule.prepare_project(&valid, &files, &config, None);
    assert_eq!(control.len(), 1);
    for source in [
        "name: app\ndependencies: [",
        "\u{feff}name: app\ndependencies: [",
    ] {
        std::fs::write(&manifest, source).unwrap();
        let malformed = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        assert_eq!(
            malformed.package(files[0].path()).unwrap().identity.name,
            "app"
        );
        assert_eq!(
            serde_json::to_value(rule.prepare_project(&malformed, &files, &config, None)).unwrap(),
            serde_json::to_value(&control).unwrap(),
        );
    }
    std::fs::write(&manifest, [0xff]).unwrap();
    let inaccessible = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    assert_eq!(
        inaccessible.package(files[0].path()).unwrap().identity.name,
        ""
    );
    assert!(
        rule.prepare_project(&inaccessible, &files, &config, None)
            .is_empty()
    );
}

fn dependency_probe_fixture() -> (tempfile::TempDir, Vec<ProjectFile>) {
    let root = tempfile::tempdir().unwrap();
    let app = root.path().join("app");
    std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
    std::fs::write(app.join("pubspec.yaml"),
        "name: app\ndependencies:\n  dep: any\n  other: any\ndev_dependencies:\n  tool: any\n  another: any\n").unwrap();
    let packages: Vec<_> = ["dep", "other", "tool", "another"]
        .into_iter()
        .map(|name| {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.join("lib")).unwrap();
            serde_json::json!({"name":name,"rootUri":format!("../../{name}"),"packageUri":"lib/"})
        })
        .collect();
    std::fs::write(
        app.join(".dart_tool/package_config.json"),
        serde_json::to_vec(&serde_json::json!({"configVersion":2,"packages":packages})).unwrap(),
    )
    .unwrap();
    let files = vec![
        ProjectFile::new(
            app.join("main.dart"),
            Arc::new(ParsedFile::parse("void main() {}")),
        )
        .unwrap(),
    ];
    (root, files)
}

fn dependency_probe_diagnostics(
    environment: &falcon_analyze::AnalysisEnvironment,
    files: &[ProjectFile],
) -> [Vec<falcon_diagnostics::Diagnostic>; 2] {
    use falcon_analyze::CrossFileRule;
    let config = FalconConfig::default();
    [
        super::unused_dependencies::UnusedDependencies.prepare_project(
            environment,
            files,
            &config,
            None,
        ),
        super::unused_dev_dependencies::UnusedDevDependencies.prepare_project(
            environment,
            files,
            &config,
            None,
        ),
    ]
}

#[test]
fn dependency_probe_unknown_is_scoped_to_its_input_and_consumer() {
    for (path, contents, expected) in [
        ("app/analysis_options.yaml", "include: [", [2, 0]),
        ("app/.flutter-plugins-dependencies", "{", [0, 2]),
        ("dep/pubspec.yaml", "flutter: [", [1, 2]),
        ("tool/pubspec.yaml", "executables: [", [2, 1]),
    ] {
        let (root, files) = dependency_probe_fixture();
        let path = root.path().join(path);
        std::fs::write(&path, contents).unwrap();
        let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        for _ in 0..2 {
            assert_eq!(
                dependency_probe_diagnostics(&environment, &files)
                    .map(|diagnostics| diagnostics.len()),
                expected,
                "{path:?}"
            );
        }
        let warnings = environment.take_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains(path.to_str().unwrap()));
    }
}

#[test]
fn dependency_probe_missing_manifests_preserve_main_behavior() {
    let (root, files) = dependency_probe_fixture();
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    assert_eq!(
        dependency_probe_diagnostics(&environment, &files).map(|diagnostics| diagnostics.len()),
        [2, 2]
    );
    assert_eq!(environment.take_warnings(), Vec::<String>::new());
    std::fs::remove_dir_all(root.path().join("tool")).unwrap();
    let missing = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    assert_eq!(
        dependency_probe_diagnostics(&missing, &files).map(|diagnostics| diagnostics.len()),
        [2, 1]
    );
}

#[test]
fn dependency_probes_keep_captured_inputs_after_deletion() {
    let (root, files) = dependency_probe_fixture();
    for (path, contents) in [
        (
            "dep/pubspec.yaml",
            "name: dep\nflutter:\n  assets: [image.png]\n",
        ),
        (
            "app/.flutter-plugins-dependencies",
            r#"{"plugins":{"ios":[{"name":"other"}]}}"#,
        ),
        ("app/analysis_options.yaml", "include: local.yaml\n"),
        ("app/local.yaml", "include: package:another/options.yaml\n"),
        ("tool/bin/command.dart", "void main() {}"),
    ] {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    let before = dependency_probe_diagnostics(&environment, &files);
    assert_eq!(before.each_ref().map(Vec::len), [0, 0]);
    for path in [
        "dep/pubspec.yaml",
        "app/.flutter-plugins-dependencies",
        "app/analysis_options.yaml",
        "app/local.yaml",
        "tool/bin/command.dart",
        "app/.dart_tool/package_config.json",
    ] {
        std::fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        serde_json::to_value(dependency_probe_diagnostics(&environment, &files)).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[test]
fn dependency_probe_resolution_uses_package_root_not_first_nested_file() {
    let (root, _) = dependency_probe_fixture();
    std::fs::write(
        root.path().join("tool/pubspec.yaml"),
        "name: tool\nflutter:\n  assets: [assets/]\n",
    )
    .unwrap();
    let nested = root.path().join("app/nested");
    std::fs::create_dir_all(nested.join(".dart_tool")).unwrap();
    std::fs::write(nested.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../../tool","packageUri":"lib/"}]}"#).unwrap();
    let files = vec![
        ProjectFile::new(
            nested.join("main.dart"),
            Arc::new(ParsedFile::parse("void main() {}")),
        )
        .unwrap(),
    ];
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    assert_eq!(
        dependency_probe_diagnostics(&environment, &files)[0].len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn dependency_probe_build_tool_preserves_symlinked_executables() {
    let (root, files) = dependency_probe_fixture();
    std::fs::create_dir(root.path().join("tool/bin")).unwrap();
    std::fs::write(root.path().join("tool/command.dart"), "void main() {}").unwrap();
    std::os::unix::fs::symlink("../command.dart", root.path().join("tool/bin/command.dart"))
        .unwrap();
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(ProjectFile::path),
        |_| None,
    );
    assert_eq!(
        dependency_probe_diagnostics(&environment, &files)[1].len(),
        1
    );
}
