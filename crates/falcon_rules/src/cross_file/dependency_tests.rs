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
    .map(|(path, source)| ProjectFile {
        path: root.path().join(path),
        parsed: Arc::new(ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parse(source).tree,
        }),
        has_parse_errors: false,
    })
    .collect();
    let config: FalconConfig = serde_json::from_value(serde_json::json!({
        "cross-file": { "rules": { "correctness": {
            "undeclared-dependencies": "warn",
            "depend-on-referenced-packages": "warn"
        } } }
    }))
    .unwrap();
    assert_eq!(
        UndeclaredDependencies
            .analyze_project("undeclared-dependencies", &files, &config, None)
            .len(),
        2
    );
    let legacy = DependOnReferencedPackages.analyze_project(
        "depend-on-referenced-packages",
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
            .analyze_project("depend-on-referenced-packages", &files, &standalone, None)
            .len(),
        3
    );
}
