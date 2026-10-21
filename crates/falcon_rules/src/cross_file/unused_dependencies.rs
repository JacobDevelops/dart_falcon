//! Report regular pubspec dependencies with no Dart import or export in the analyzed package.
//!
//! All analyzed Dart locations count, including bin, test, tool and examples, as
//! do conditional import/export alternatives. Dev-dependencies, SDK dependencies
//! and plugins registered in `.flutter-plugins-dependencies` are exempt: native
//! registration can use a plugin without a Dart import. Declared dependencies
//! whose resolved pubspec declares Flutter fonts, assets or plugin metadata are
//! also exempt, since Flutter can consume them without a Dart import. Resolution
//! uses the nearest `.dart_tool/package_config.json`; missing or unreadable
//! dependency metadata cannot establish this exemption. Nested packages are
//! independent. This is a source-set check, not a pub solver: excluded/unscanned
//! files, build-time tools and native-only packages without Flutter metadata
//! cannot establish usage. Suppress intentional non-Dart dependencies in pubspec.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, PackageConfig, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use marked_yaml::{LoaderOptions, parse_yaml_with_options};

use super::depend_on_referenced_packages::{imported_package, read_manifest};
use super::{character_to_byte, enclosing_pubspec, program_reference_uris, scalar_source_span};

#[derive(Default)]
pub struct UnusedDependencies;

impl CrossFileRule for UnusedDependencies {
    fn prepare_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut usage: BTreeMap<PathBuf, HashSet<String>> = BTreeMap::new();
        for file in files {
            let Some(pubspec) = enclosing_pubspec(&file.path) else {
                continue;
            };
            let used = usage.entry(pubspec).or_default();
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("dart")
            {
                continue;
            }
            for uri in program_reference_uris(file.program()) {
                if let Some(package) = imported_package(&uri) {
                    used.insert(package.to_owned());
                }
            }
        }
        let mut diagnostics = Vec::new();
        for (pubspec, mut used) in usage {
            if read_manifest(&pubspec).is_none() {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&pubspec) else {
                continue;
            };
            let Ok(root) = parse_yaml_with_options(
                0,
                &source,
                LoaderOptions::default().error_on_duplicate_keys(true),
            ) else {
                continue;
            };
            let Some(dependencies) = root
                .as_mapping()
                .and_then(|root| root.get_mapping("dependencies"))
            else {
                continue;
            };
            let package_root = pubspec.parent().unwrap();
            used.extend(native_plugins(package_root));
            let package_config =
                PackageConfig::discover(package_root).and_then(|path| PackageConfig::load(&path));
            for (name, value) in dependencies.iter() {
                if used.contains(name.as_str())
                    || value
                        .as_mapping()
                        .is_some_and(|mapping| mapping.get("sdk").is_some())
                    || package_config
                        .as_ref()
                        .and_then(|config| config.package(name.as_str()))
                        .is_some_and(|package| has_flutter_resources(&package.root))
                {
                    continue;
                }
                let Some(start) = name
                    .span()
                    .start()
                    .and_then(|marker| character_to_byte(&source, marker.character()))
                else {
                    continue;
                };
                let Some(span) = scalar_source_span(&source, start) else {
                    continue;
                };
                let mut diagnostic = Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!(
                        "No analyzed Dart source uses '{}'; remove the dependency if it is not used natively or by build tools.",
                        name.as_str()
                    ),
                    pubspec.to_string_lossy().into_owned(),
                    span,
                );
                diagnostic.resolve_position(&source);
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }
}

fn native_plugins(root: &Path) -> HashSet<String> {
    let Some(plugins) = std::fs::read_to_string(root.join(".flutter-plugins-dependencies"))
        .ok()
        .and_then(|source| serde_json::from_str::<serde_json::Value>(&source).ok())
    else {
        return HashSet::new();
    };
    plugins
        .get("plugins")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|platforms| platforms.values())
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(|plugin| plugin.get("name")?.as_str())
        .map(ToOwned::to_owned)
        .collect()
}

fn has_flutter_resources(root: &Path) -> bool {
    let Some(manifest) = std::fs::read_to_string(root.join("pubspec.yaml"))
        .ok()
        .and_then(|source| serde_yaml::from_str::<serde_yaml::Value>(&source).ok())
    else {
        return false;
    };
    let Some(flutter) = manifest
        .get("flutter")
        .and_then(serde_yaml::Value::as_mapping)
    else {
        return false;
    };
    ["fonts", "assets"].iter().any(|key| {
        flutter
            .get(serde_yaml::Value::String((*key).to_owned()))
            .and_then(serde_yaml::Value::as_sequence)
            .is_some_and(|entries| !entries.is_empty())
    }) || flutter
        .get(serde_yaml::Value::String("plugin".to_owned()))
        .and_then(serde_yaml::Value::as_mapping)
        .is_some_and(|plugin| !plugin.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::CrossFileRuleAnalysis;

    #[test]
    fn package_config_resolves_implicitly_used_font_package() {
        let directory = tempfile::tempdir().unwrap();
        let app = directory.path().join("app");
        let fonts = directory.path().join("arbitrary_font_bundle");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(&fonts).unwrap();
        std::fs::write(
            app.join("pubspec.yaml"),
            "name: app\ndependencies:\n  arbitrary_font_bundle: any\n",
        )
        .unwrap();
        std::fs::write(
            fonts.join("pubspec.yaml"),
            "name: arbitrary_font_bundle\nflutter:\n  fonts:\n    - family: CustomIcons\n      fonts:\n        - asset: fonts/icons.ttf\n",
        )
        .unwrap();
        std::fs::write(
            app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"arbitrary_font_bundle","rootUri":"../../arbitrary_font_bundle","packageUri":"lib/"}]}"#,
        )
        .unwrap();
        let config = PackageConfig::load(&PackageConfig::discover(&app).unwrap()).unwrap();
        let package = config.package("arbitrary_font_bundle").unwrap();
        assert!(has_flutter_resources(&package.root));
        let source = "void main() {}";
        let files = [ProjectFile {
            path: app.join("main.dart"),
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: falcon_dart_parser::parse(source).tree,
            }),
            has_parse_errors: false,
        }];
        assert!(
            UnusedDependencies
                .analyze_project("unused-dependencies", &files, &FalconConfig::default(), None)
                .is_empty()
        );
        std::fs::write(
            fonts.join("pubspec.yaml"),
            "name: arbitrary_font_bundle\nflutter:\n  fonts: []\n  assets: []\n  plugin: {}\n",
        )
        .unwrap();
        assert!(!has_flutter_resources(&package.root));
        assert_eq!(
            UnusedDependencies
                .analyze_project("unused-dependencies", &files, &FalconConfig::default(), None)
                .len(),
            1
        );
    }
}
