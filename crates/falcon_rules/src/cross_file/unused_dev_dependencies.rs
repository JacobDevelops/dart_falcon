//! Report resolved pubspec dev dependencies unused by analyzed Dart sources or tools.
//!
//! This rule needs `pub get`: unresolved dependencies are not reported. A package
//! is checked only when at least one analyzed Dart file is outside `lib/`, since
//! library-only analysis cannot establish non-use of development tools. Imports
//! and exports in every analyzed location and conditional alternatives count.
//! Deliberately, `lib/` imports also count: `depend-on-referenced-packages` covers
//! dev dependencies referenced by library code.
//!
//! SDK dependencies, analysis-options includes and analyzer plugins, same-named
//! top-level pubspec configuration, builders, executables and `bin/*.dart` tools
//! are exempt. When `custom_lint` is enabled, resolved packages depending on
//! `custom_lint_builder` are exempt too. Options are discovered at the package or
//! nearest ancestor; relative scalar/list includes are followed with cycle
//! protection. Package includes establish usage but are not followed. Resolution
//! uses the package root's nearest `.dart_tool/package_config.json`. Missing tool
//! metadata does not establish an exemption (an absent resolved package directory
//! remains exempt); malformed or inaccessible probe inputs warn and conservatively
//! skip only the dependent check. Nested packages are independent, and excluded or
//! unscanned sources cannot establish usage.

use std::collections::HashSet;
use std::path::Path;

use falcon_analyze::{
    AnalysisEnvironment, CrossFileRule, DependencyInput, ManifestSnapshot, ProjectFile,
    canonical_package_path,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use serde_yaml::Value;

use super::{analyzed_package_usage, unused_dependency_diagnostics};
use crate::lint::support::package_manifests::{imported_package, is_library_file};

#[derive(Default)]
pub struct UnusedDevDependencies;

impl CrossFileRule for UnusedDevDependencies {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let non_library_packages: HashSet<_> = files
            .iter()
            .filter(|file| {
                file.path()
                    .extension()
                    .is_some_and(|extension| extension == "dart")
            })
            .filter_map(|file| {
                let pubspec = &environment.package(file.path())?.manifest;
                (!is_library_file(file.path(), pubspec)).then_some(pubspec.clone())
            })
            .collect();
        let mut diagnostics = Vec::new();
        for (pubspec, (package, mut used)) in analyzed_package_usage(environment, files) {
            if !non_library_packages.contains(&pubspec) {
                continue;
            }
            let Some(package_root) = pubspec.parent() else {
                continue;
            };
            let Some(options) = analysis_options_packages(environment, package_root) else {
                continue;
            };
            let custom_lint = options.contains("custom_lint");
            used.extend(options);
            diagnostics.extend(unused_dependency_diagnostics(
                environment,
                package,
                &used,
                "dev_dependencies",
                |name, root, resolved| {
                    root.get(name).is_some()
                        || resolved.is_none_or(|root| is_build_tool(environment, root, custom_lint))
                },
                |name| format!(
                    "No analyzed Dart source uses `{name}`; remove the dev dependency if it is not used by build tools."
                ),
            ));
        }
        diagnostics
    }
}

fn analysis_options_packages(
    environment: &AnalysisEnvironment,
    root: &Path,
) -> Option<HashSet<String>> {
    let mut packages = HashSet::new();
    let mut pending = Vec::new();
    if let Some(path) = environment.dependency_ancestor_file(root, "analysis_options.yaml") {
        pending.push(path);
    }
    let mut visited = HashSet::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(canonical_package_path(&path)) {
            continue;
        }
        if visited.len() > 4096 {
            environment.warn_optional_metadata(
                &path,
                &path,
                "analysis options include graph exceeds 4096 documents",
            );
            return None;
        }
        let options = match environment.dependency_yaml(&path) {
            DependencyInput::Present(options) => options,
            DependencyInput::Missing => continue,
            DependencyInput::Unknown(_) => return None,
        };
        if let Some(plugins) = options.get("plugins") {
            plugin_packages(plugins, &mut packages);
        }
        if let Some(plugins) = options
            .get("analyzer")
            .and_then(|analyzer| analyzer.get("plugins"))
        {
            plugin_packages(plugins, &mut packages);
        }
        for include in options.get("include").into_iter().flat_map(|include| {
            include
                .as_sequence()
                .into_iter()
                .flatten()
                .chain(include.as_str().map(|_| include))
        }) {
            let Some(include) = include.as_str() else {
                continue;
            };
            if let Some(package) = imported_package(include) {
                packages.insert(package.to_owned());
            } else if !include.contains(':')
                && let Some(parent) = path.parent()
            {
                pending.push(parent.join(include));
            }
        }
    }
    Some(packages)
}

fn plugin_packages(plugins: &Value, packages: &mut HashSet<String>) {
    let names = plugins
        .as_mapping()
        .into_iter()
        .flat_map(|mapping| mapping.keys())
        .chain(plugins.as_sequence().into_iter().flatten())
        .chain(plugins.as_str().map(|_| plugins));
    packages.extend(names.filter_map(Value::as_str).map(ToOwned::to_owned));
}

fn is_build_tool(environment: &AnalysisEnvironment, root: &Path, custom_lint: bool) -> bool {
    match environment.dependency_is_directory(root) {
        DependencyInput::Present(true) => {}
        DependencyInput::Missing
        | DependencyInput::Unknown(_)
        | DependencyInput::Present(false) => return true,
    }
    match environment.dependency_readable(&root.join("build.yaml")) {
        DependencyInput::Present(()) | DependencyInput::Unknown(_) => return true,
        DependencyInput::Missing => {}
    }
    match environment.dependency_directory(&root.join("bin"), "dart") {
        DependencyInput::Present(true) | DependencyInput::Unknown(_) => return true,
        DependencyInput::Missing | DependencyInput::Present(false) => {}
    }
    let package = environment.dependency_manifest(root);
    match package.snapshot() {
        ManifestSnapshot::Missing => false,
        ManifestSnapshot::Malformed(_) | ManifestSnapshot::Inaccessible(_) => true,
        ManifestSnapshot::Valid { yaml: manifest, .. } => {
            manifest
                .get("executables")
                .and_then(Value::as_mapping)
                .is_some_and(|entries| !entries.is_empty())
                || (custom_lint
                    && manifest
                        .get("dependencies")
                        .and_then(Value::as_mapping)
                        .is_some_and(|dependencies| {
                            dependencies.contains_key("custom_lint_builder")
                        }))
        }
    }
}
