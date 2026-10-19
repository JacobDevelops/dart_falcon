//! Parsed SDK and dependency source staging shared by corpus tests and xtask.

use std::fs;
use std::path::{Path, PathBuf};

// Declaration-only excerpts: inherited and generic member types are resolved by
// the normal parser and indexes, never a second handwritten signature table.
const CORE: &str = include_str!("sdk/lib/core/core.dart");
const COLLECTION: &str = include_str!("sdk/lib/collection/collection.dart");

#[allow(dead_code)]
pub fn requires_sdk(file: &Path) -> bool {
    fixture_list_contains(file, include_str!("sdk/fixtures.txt"))
}

pub fn fixture_list_contains(file: &Path, fixtures: &str) -> bool {
    fixtures
        .lines()
        .map(str::trim)
        .filter(|fixture| !fixture.is_empty())
        .any(|fixture| file.ends_with(fixture))
}

/// Package searches stop at the fixture's rule, never an enclosing checkout.
#[allow(dead_code)]
pub fn source_directory(file: &Path) -> &Path {
    let parent = file.parent().unwrap();
    let in_corpus = file
        .ancestors()
        .any(|path| path.file_name().is_some_and(|name| name == "corpus"));
    for directory in file.ancestors().skip(1) {
        if directory.join("pubspec.yaml").is_file() {
            return directory;
        }
        if !in_corpus
            || directory
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "corpus")
        {
            break;
        }
    }
    parent
}

#[allow(dead_code)]
pub fn staged_directory(project: &Path, source: &Path) -> PathBuf {
    project.join(if source.join("pubspec.yaml").is_file() {
        "app"
    } else {
        "app/lib"
    })
}

#[allow(dead_code)]
pub fn staged_file(project: &Path, source: &Path, file: &Path) -> PathBuf {
    falcon_analyze::canonical_or_lexical(
        &staged_directory(project, source).join(file.strip_prefix(source).unwrap()),
    )
}

pub fn sdk_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
    fs::create_dir_all(sdk.join("core")).unwrap();
    fs::create_dir_all(sdk.join("collection")).unwrap();
    fs::create_dir_all(sdk.join("async")).unwrap();
    fs::create_dir_all(sdk.join("math")).unwrap();
    fs::create_dir_all(sdk.join("convert")).unwrap();
    fs::write(
        sdk.join("libraries.json"),
        include_str!("sdk/lib/libraries.json"),
    )
    .unwrap();
    fs::write(sdk.join("core/core.dart"), CORE).unwrap();
    fs::write(sdk.join("collection/collection.dart"), COLLECTION).unwrap();
    fs::write(
        sdk.join("async/async.dart"),
        include_str!("sdk/lib/async/async.dart"),
    )
    .unwrap();
    fs::write(
        sdk.join("math/math.dart"),
        include_str!("sdk/lib/math/math.dart"),
    )
    .unwrap();
    fs::write(
        sdk.join("convert/convert.dart"),
        include_str!("sdk/lib/convert/convert.dart"),
    )
    .unwrap();
    fs::create_dir_all(root.join("app/.dart_tool")).unwrap();
    fs::create_dir_all(root.join("app/lib")).unwrap();
    fs::write(root.join("app/pubspec.yaml"), "name: app\n").unwrap();
    fs::create_dir_all(root.join("meta/lib")).unwrap();
    fs::write(
        root.join("meta/lib/meta.dart"),
        "library meta;\nconst Object internal = _Internal();\n\
         class _Internal { const _Internal(); }\n\
         const Object visibleForTesting = _VisibleForTesting();\n\
         class _VisibleForTesting { const _VisibleForTesting(); }\n\
         const Object protected = _Protected();\n\
         class _Protected { const _Protected(); }\n\
         const Object awaitNotRequired = _AwaitNotRequired();\n\
         class _AwaitNotRequired { const _AwaitNotRequired(); }\n\
         const Immutable immutable = Immutable();\n\
         class Immutable { const Immutable([this.reason]); final String? reason; }\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("flutter/packages/flutter/lib")).unwrap();
    fs::write(
        root.join("flutter/packages/flutter/lib/widgets.dart"),
        "export 'package:meta/meta.dart' show visibleForTesting;\n\
         export 'foundation.dart';\n\
         export 'src_widgets.dart';\n",
    )
    .unwrap();
    fs::write(
        root.join("flutter/packages/flutter/lib/src_widgets.dart"),
        "import 'package:meta/meta.dart';\n\
         import 'foundation.dart';\n\
         abstract class Key { const Key(); }\n\
         abstract class BuildContext { bool get mounted; }\n\
         @immutable\n\
         abstract class Widget extends DiagnosticableTree { const Widget({this.key}); final Key? key; }\n\
         class SizedBox extends Widget { const SizedBox({super.key, this.width, this.height, this.child}); final double? width; final double? height; final Widget? child; }\n\
         class Text extends Widget { const Text(String data, {super.key}); }\n\
         class Center extends Widget { const Center({super.key, this.child}); final Widget? child; }\n\
         class Column extends Widget { const Column({super.key, this.children = const []}); final List<Widget> children; }\n\
         class EdgeInsets { const EdgeInsets.all(double value); static const zero = EdgeInsets.all(0); }\n\
         class Padding extends Widget { const Padding({super.key, required this.padding, required this.child}); final EdgeInsets padding; final Widget child; }\n\
         class Container extends Widget { Container({super.key, this.child, this.width}); final Widget? child; final double? width; }\n\
         abstract class StatelessWidget extends Widget { const StatelessWidget({super.key}); }\n\
         abstract class StatefulWidget extends Widget { const StatefulWidget({super.key}); }\n\
         abstract class State<T extends StatefulWidget> with Diagnosticable { bool get mounted; T get widget; }\n\
         class Placeholder extends Widget { const Placeholder({super.key}); }\n",
    )
    .unwrap();
    fs::write(
        root.join("flutter/packages/flutter/lib/material.dart"),
        "export 'widgets.dart';\n",
    )
    .unwrap();
    fs::write(
        root.join("flutter/packages/flutter/lib/foundation.dart"),
        "export 'package:meta/meta.dart' show visibleForTesting;\n\
         export 'src/foundation/constants.dart';\n\
         mixin Diagnosticable { void debugFillProperties(DiagnosticPropertiesBuilder properties) {} }\n\
         abstract class DiagnosticableTree with Diagnosticable { const DiagnosticableTree(); }\n\
         mixin DiagnosticableTreeMixin implements DiagnosticableTree {}\n\
         class DiagnosticPropertiesBuilder { void add(Object property) {} }\n\
         class DiagnosticsProperty<T> { DiagnosticsProperty(String name, T value); }\n\
         class IntProperty extends DiagnosticsProperty<int> { IntProperty(super.name, super.value); }\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("flutter/packages/flutter/lib/src/foundation")).unwrap();
    fs::write(
        root.join("flutter/packages/flutter/lib/src/foundation/constants.dart"),
        "const bool kDebugMode = true;\n",
    )
    .unwrap();
    let mut flutter_root = String::from("file://");
    falcon_analyze::environment::percent_encode(
        root.join("flutter").to_str().unwrap(),
        &mut flutter_root,
    );
    fs::write(
        root.join("app/.dart_tool/package_config.json"),
        serde_json::json!({
            "configVersion": 2,
            "flutterRoot": flutter_root,
            "packages": [
                {"name": "app", "rootUri": "../", "packageUri": "lib/"},
                {"name": "meta", "rootUri": "../../meta", "packageUri": "lib/"},
                {"name": "flutter", "rootUri": "../../flutter/packages/flutter", "packageUri": "lib/"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    dir
}

/// Keep relative imports and package-local source declarations alongside the
/// analyzed file, while the temporary package supplies only external metadata.
pub(super) fn stage_sources(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    let package_config = source.join(".dart_tool/package_config.json");
    if package_config.is_file() {
        fs::create_dir_all(destination.join(".dart_tool")).unwrap();
        fs::copy(
            package_config,
            destination.join(".dart_tool/package_config.json"),
        )
        .unwrap();
    }
    for entry in fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if !path.file_name().unwrap().to_string_lossy().starts_with('.') {
                stage_sources(&path, &destination.join(path.file_name().unwrap()));
            }
        } else if path
            .extension()
            .is_some_and(|extension| extension == "dart")
            || path.file_name().is_some_and(|name| name == "pubspec.yaml")
        {
            fs::copy(&path, destination.join(path.file_name().unwrap())).unwrap();
        }
    }
}

#[allow(dead_code)]
pub fn stage_project(source: &Path) -> tempfile::TempDir {
    let project = sdk_project();
    let destination = staged_directory(project.path(), source);
    if source.join("pubspec.yaml").is_file() {
        fs::remove_file(project.path().join("app/.dart_tool/package_config.json")).unwrap();
    }
    stage_sources(source, &destination);
    configure_packages(&project.path().join("app"));
    project
}

fn file_uri(path: &Path) -> String {
    let path = falcon_analyze::canonical_or_lexical(path);
    let mut uri = String::from("file://");
    falcon_analyze::environment::percent_encode(path.to_str().unwrap(), &mut uri);
    uri.push('/');
    uri
}

fn configure_packages(app: &Path) {
    fn collect(
        directory: &Path,
        packages: &mut Vec<(String, PathBuf)>,
        configurations: &mut Vec<PathBuf>,
    ) {
        let pubspec = directory.join("pubspec.yaml");
        if let Ok(source) = fs::read_to_string(&pubspec)
            && let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&source)
            && let Some(name) = yaml.get("name").and_then(serde_yaml::Value::as_str)
        {
            packages.push((name.to_owned(), directory.to_path_buf()));
        }
        if pubspec.is_file() || directory.join(".dart_tool/package_config.json").is_file() {
            configurations.push(directory.to_path_buf());
        }
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() && !path.file_name().unwrap().to_string_lossy().starts_with('.') {
                collect(&path, packages, configurations);
            }
        }
    }
    let mut packages = Vec::new();
    let mut configurations = Vec::new();
    collect(app, &mut packages, &mut configurations);
    let root = app.parent().unwrap();
    let mut entries: Vec<_> = packages.iter().map(|(name, path)| {
        serde_json::json!({"name": name, "rootUri": file_uri(path), "packageUri": "lib/"})
    }).collect();
    entries.extend([
        serde_json::json!({"name": "meta", "rootUri": file_uri(&root.join("meta")), "packageUri": "lib/"}),
        serde_json::json!({"name": "flutter", "rootUri": file_uri(&root.join("flutter/packages/flutter")), "packageUri": "lib/"}),
    ]);
    for directory in &configurations {
        fs::create_dir_all(directory.join(".dart_tool")).unwrap();
        let config_path = directory.join(".dart_tool/package_config.json");
        let mut config = match fs::read_to_string(&config_path) {
            Ok(source) => match serde_json::from_str::<serde_json::Value>(&source) {
                Ok(config) => config,
                // Keep intentionally malformed fixture metadata intact.
                Err(_) => continue,
            },
            Err(_) => serde_json::json!({"configVersion": 2, "packages": []}),
        };
        if !config
            .get("packages")
            .is_some_and(serde_json::Value::is_array)
        {
            continue;
        }
        let existing = falcon_analyze::PackageConfig::load(&config_path);
        config["flutterRoot"] = serde_json::Value::String(file_uri(&root.join("flutter")));
        let Some(configured) = config["packages"].as_array_mut() else {
            continue;
        };
        for entry in &entries {
            let same_root = existing.as_ref().is_some_and(|config| {
                config
                    .packages
                    .iter()
                    .any(|package| entry["rootUri"] == file_uri(&package.root))
            });
            if !same_root
                && !configured
                    .iter()
                    .any(|existing| existing["name"] == entry["name"])
            {
                configured.push(entry.clone());
            }
        }
        fs::write(config_path, config.to_string()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_keeps_sources_and_configures_parsed_dependencies() {
        let source = tempfile::tempdir().unwrap();
        fs::create_dir_all(source.path().join("lib/src")).unwrap();
        fs::write(source.path().join("pubspec.yaml"), "name: sample\n").unwrap();
        fs::write(
            source.path().join("lib/main.dart"),
            "import 'src/local.dart';\nimport 'package:flutter/widgets.dart';\n",
        )
        .unwrap();
        fs::write(source.path().join("lib/src/local.dart"), "class Local {}").unwrap();
        let project = stage_project(source.path());
        let app = project.path().join("app");
        assert_eq!(
            fs::read_to_string(app.join("lib/src/local.dart")).unwrap(),
            "class Local {}",
        );
        let config =
            falcon_analyze::PackageConfig::load(&app.join(".dart_tool/package_config.json"))
                .unwrap();
        for uri in [
            "package:meta/meta.dart",
            "package:flutter/widgets.dart",
            "package:flutter/foundation.dart",
        ] {
            assert!(config.resolve_package_uri(uri).unwrap().is_file(), "{uri}");
        }
        let widgets = config
            .resolve_package_uri("package:flutter/widgets.dart")
            .unwrap();
        assert!(
            fs::read_to_string(widgets)
                .unwrap()
                .contains("export 'foundation.dart'"),
        );
    }

    #[test]
    fn staging_preserves_package_names_and_nested_boundaries() {
        let source = tempfile::tempdir().unwrap();
        for (directory, name) in [("", "own_package"), ("external", "dependency")] {
            let package = source.path().join(directory);
            fs::create_dir_all(package.join("lib")).unwrap();
            fs::write(package.join("pubspec.yaml"), format!("name: {name}\n")).unwrap();
            fs::write(package.join("lib/value.dart"), "class Value {}").unwrap();
        }
        fs::create_dir(source.path().join(".dart_tool")).unwrap();
        fs::write(source.path().join(".dart_tool/package_config.json"), serde_json::json!({
            "configVersion": 2,
            "packages": [{"name": "own_package", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.9"}],
        }).to_string()).unwrap();
        let project = stage_project(source.path());
        for directory in ["app", "app/external"] {
            let config = falcon_analyze::PackageConfig::load(
                &project
                    .path()
                    .join(directory)
                    .join(".dart_tool/package_config.json"),
            )
            .unwrap();
            for uri in [
                "package:own_package/value.dart",
                "package:dependency/value.dart",
            ] {
                assert!(config.resolve_package_uri(uri).unwrap().is_file());
            }
        }
        let root_config = falcon_analyze::PackageConfig::load(
            &project.path().join("app/.dart_tool/package_config.json"),
        )
        .unwrap();
        assert!(
            root_config
                .resolve_package_uri("package:own_package/value.dart")
                .unwrap()
                .is_file()
        );
        assert_eq!(
            root_config
                .packages
                .iter()
                .filter(|package| package.name == "own_package")
                .count(),
            1
        );
        assert_eq!(
            fs::read_to_string(project.path().join("app/pubspec.yaml")).unwrap(),
            "name: own_package\n"
        );
        assert_eq!(
            fs::read_to_string(project.path().join("app/external/pubspec.yaml")).unwrap(),
            "name: dependency\n"
        );
    }

    #[test]
    fn unpackaged_fixture_stays_under_lib_and_search_stops_at_rule() {
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("pubspec.yaml"), "name: unrelated\n").unwrap();
        let rule = source.path().join("corpus/rule");
        fs::create_dir_all(&rule).unwrap();
        let file = rule.join("main.dart");
        fs::write(&file, "class Local {}").unwrap();
        assert_eq!(source_directory(&file), rule);
        let project = stage_project(&rule);
        assert!(
            staged_directory(project.path(), &rule)
                .join("main.dart")
                .is_file()
        );
        assert!(project.path().join("app/lib/main.dart").is_file());
    }

    #[test]
    fn package_config_without_pubspec_gets_pinned_metadata() {
        let source = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join(".dart_tool")).unwrap();
        fs::write(source.path().join("main.dart"), "class Local {}").unwrap();
        fs::write(
            source.path().join(".dart_tool/package_config.json"),
            serde_json::json!({
                "configVersion": 2,
                "packages": [{"name": "fixture", "rootUri": "../", "packageUri": ""}],
            })
            .to_string(),
        )
        .unwrap();
        fs::create_dir(source.path().join("malformed")).unwrap();
        fs::write(source.path().join("malformed/pubspec.yaml"), "name: [\n").unwrap();
        let project = stage_project(source.path());
        let config = falcon_analyze::PackageConfig::load(
            &project
                .path()
                .join("app/lib/.dart_tool/package_config.json"),
        )
        .unwrap();
        assert!(
            config
                .flutter_root
                .as_ref()
                .unwrap()
                .join("bin/cache/dart-sdk/lib/core/core.dart")
                .is_file()
        );
        assert!(
            config
                .resolve_package_uri("package:fixture/main.dart")
                .unwrap()
                .is_file()
        );
        assert!(
            config
                .resolve_package_uri("package:meta/meta.dart")
                .unwrap()
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(project.path().join("app/lib/malformed/pubspec.yaml")).unwrap(),
            "name: [\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn staged_project_paths_are_canonical_through_symlinked_roots() {
        let source = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("actual")).unwrap();
        fs::write(source.path().join("actual/main.dart"), "class Local {}").unwrap();
        std::os::unix::fs::symlink(source.path().join("actual"), source.path().join("alias"))
            .unwrap();
        let project = stage_project(&source.path().join("alias"));
        std::os::unix::fs::symlink(project.path(), source.path().join("staged_alias")).unwrap();
        let original = source.path().join("alias/main.dart");
        let through_alias = staged_file(
            &source.path().join("staged_alias"),
            &source.path().join("alias"),
            &original,
        );
        assert_eq!(
            through_alias,
            project
                .path()
                .join("app/lib/main.dart")
                .canonicalize()
                .unwrap(),
        );
    }
}
