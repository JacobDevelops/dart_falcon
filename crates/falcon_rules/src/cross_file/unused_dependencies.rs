//! Report regular pubspec dependencies with no Dart import or export in the analyzed package.
//!
//! All analyzed Dart locations count, including bin, test, tool and examples, as
//! do conditional import/export alternatives. Dev-dependencies, SDK dependencies
//! and plugins registered in `.flutter-plugins-dependencies` are exempt: native
//! registration can use a plugin without a Dart import. Declared dependencies
//! whose resolved pubspec declares Flutter fonts, assets or plugin metadata are
//! also exempt, since Flutter can consume them without a Dart import. Resolution
//! uses the package root's nearest `.dart_tool/package_config.json`. Missing
//! dependency manifests do not establish an exemption; malformed or inaccessible
//! probe inputs warn and conservatively skip only the dependent check. Nested packages are
//! independent. This is a source-set check, not a pub solver: excluded/unscanned
//! files, build-time tools and native-only packages without Flutter metadata
//! cannot establish usage. Suppress intentional non-Dart dependencies in pubspec.

use std::collections::HashSet;
use std::path::Path;

use falcon_analyze::{
    AnalysisEnvironment, CrossFileRule, DependencyInput, ManifestSnapshot, ProjectFile,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

use super::{analyzed_package_usage, unused_dependency_diagnostics};

#[derive(Default)]
pub struct UnusedDependencies;

impl CrossFileRule for UnusedDependencies {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (pubspec, (package, mut used)) in analyzed_package_usage(environment, files) {
            if let Some(package_root) = pubspec.parent() {
                match native_plugins(environment, package_root) {
                    Some(plugins) => used.extend(plugins),
                    None => continue,
                }
            }
            diagnostics.extend(unused_dependency_diagnostics(
                environment,
                package,
                &used,
                "dependencies",
                |_, _, resolved| resolved.is_some_and(|root| has_flutter_resources(environment, root)),
                |name| format!(
                    "No analyzed Dart source uses `{name}`; remove the dependency if it is not used natively or by build tools."
                ),
            ));
        }
        diagnostics
    }
}

fn native_plugins(environment: &AnalysisEnvironment, root: &Path) -> Option<HashSet<String>> {
    let plugins = match environment.dependency_json(&root.join(".flutter-plugins-dependencies")) {
        DependencyInput::Present(plugins) => plugins,
        DependencyInput::Missing => return Some(HashSet::new()),
        DependencyInput::Unknown(_) => return None,
    };
    Some(
        plugins
            .get("plugins")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|platforms| platforms.values())
            .filter_map(serde_json::Value::as_array)
            .flatten()
            .filter_map(|plugin| plugin.get("name")?.as_str())
            .map(ToOwned::to_owned)
            .collect(),
    )
}

fn has_flutter_resources(environment: &AnalysisEnvironment, root: &Path) -> bool {
    let package = environment.dependency_manifest(root);
    let manifest = match package.snapshot() {
        ManifestSnapshot::Valid { yaml, .. } => yaml,
        ManifestSnapshot::Missing => return false,
        ManifestSnapshot::Malformed(_) | ManifestSnapshot::Inaccessible(_) => return true,
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
    use falcon_analyze::{CrossFileRuleAnalysis, PackageConfig};

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
        assert!(has_flutter_resources(
            &AnalysisEnvironment::default(),
            &package.root
        ));
        let source = "void main() {}";
        let files = [ProjectFile::new(
            app.join("main.dart"),
            std::sync::Arc::new(falcon_analyze::ParsedFile::parse(source)),
        )
        .expect("parsed fixture must be accepted by the project")];
        assert!(
            UnusedDependencies
                .analyze_project(
                    "unused-dependencies",
                    &falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
                        files.iter().map(ProjectFile::path),
                        |_| None,
                    ),
                    &files,
                    &FalconConfig::default(),
                    None,
                )
                .is_empty()
        );
        std::fs::write(
            fonts.join("pubspec.yaml"),
            "name: arbitrary_font_bundle\nflutter:\n  fonts: []\n  assets: []\n  plugin: {}\n",
        )
        .unwrap();
        assert!(!has_flutter_resources(
            &AnalysisEnvironment::default(),
            &package.root
        ));
        assert_eq!(
            UnusedDependencies
                .analyze_project(
                    "unused-dependencies",
                    &falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
                        files.iter().map(ProjectFile::path),
                        |_| None,
                    ),
                    &files,
                    &FalconConfig::default(),
                    None,
                )
                .len(),
            1
        );
    }
}
