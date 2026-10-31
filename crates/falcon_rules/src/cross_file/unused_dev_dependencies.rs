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
//! uses the nearest `.dart_tool/package_config.json`. Missing or unreadable tool
//! metadata cannot establish an exemption. Nested packages are independent, and
//! excluded or unscanned sources cannot establish usage.

use std::collections::HashSet;
use std::path::Path;

use falcon_analyze::{CrossFileRule, ProjectFile, canonical_package_path};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use serde_yaml::Value;

use super::{analyzed_package_usage, enclosing_pubspec, unused_dependency_diagnostics};
use crate::lint::support::package_manifests::{imported_package, is_library_file};

#[derive(Default)]
pub struct UnusedDevDependencies;

impl CrossFileRule for UnusedDevDependencies {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let non_library_packages: HashSet<_> = files
            .iter()
            .filter(|file| {
                file.path
                    .extension()
                    .is_some_and(|extension| extension == "dart")
            })
            .filter_map(|file| {
                let pubspec = enclosing_pubspec(&file.path)?;
                (!is_library_file(&file.path, &pubspec)).then_some(pubspec)
            })
            .collect();
        let mut diagnostics = Vec::new();
        for (pubspec, mut used) in analyzed_package_usage(files) {
            if !non_library_packages.contains(&pubspec) {
                continue;
            }
            let Some(package_root) = pubspec.parent() else {
                continue;
            };
            let options = analysis_options_packages(package_root);
            let custom_lint = options.contains("custom_lint");
            used.extend(options);
            diagnostics.extend(unused_dependency_diagnostics(
                &pubspec,
                &used,
                "dev_dependencies",
                |name, root, resolved| {
                    root.get(name).is_some()
                        || resolved
                            .is_none_or(|root| !root.is_dir() || is_build_tool(root, custom_lint))
                },
                |name| format!(
                    "No analyzed Dart source uses `{name}`; remove the dev dependency if it is not used by build tools."
                ),
            ));
        }
        diagnostics
    }
}

fn read_yaml(path: &Path) -> Option<Value> {
    let source = falcon_config::read_metadata_text(path).ok()?;
    serde_yaml::from_str(&source).ok()
}

fn analysis_options_packages(root: &Path) -> HashSet<String> {
    let mut packages = HashSet::new();
    let Some(options) = root
        .ancestors()
        .map(|ancestor| ancestor.join("analysis_options.yaml"))
        .find(|path| path.is_file())
    else {
        return packages;
    };
    let mut pending = vec![options];
    let mut visited = HashSet::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(canonical_package_path(&path)) {
            continue;
        }
        let Some(options) = read_yaml(&path) else {
            continue;
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
    packages
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

fn is_build_tool(root: &Path, custom_lint: bool) -> bool {
    if falcon_config::read_metadata_text(&root.join("build.yaml")).is_ok()
        || std::fs::read_dir(root.join("bin")).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                let path = entry.path();
                path.extension()
                    .is_some_and(|extension| extension == "dart")
                    && path.is_file()
            })
        })
    {
        return true;
    }
    read_yaml(&root.join("pubspec.yaml")).is_some_and(|manifest| {
        manifest
            .get("executables")
            .and_then(Value::as_mapping)
            .is_some_and(|entries| !entries.is_empty())
            || (custom_lint
                && manifest
                    .get("dependencies")
                    .and_then(Value::as_mapping)
                    .is_some_and(|dependencies| dependencies.contains_key("custom_lint_builder")))
    })
}
