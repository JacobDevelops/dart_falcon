//! Report package imports and exports missing from the enclosing pubspec.
//!
//! Regular dependencies and self-imports are allowed everywhere; dev-dependencies
//! are allowed only outside `lib/`. Generated `flutter_gen` imports are exempt.
//! Malformed manifests are skipped; conditional alternatives are checked too.
//! `depend-on-referenced-packages` defers overlapping reports when this rule is
//! enabled, but retains its stricter bin/build-hook checks.

use std::collections::HashMap;

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::depend_on_referenced_packages::{imported_package, is_library_file, read_manifest};
use super::enclosing_pubspec;

#[derive(Default)]
pub struct UndeclaredDependencies;

impl CrossFileRule for UndeclaredDependencies {
    fn prepare_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut manifests = HashMap::new();
        let mut diagnostics = Vec::new();
        for file in files {
            let Some(pubspec) = enclosing_pubspec(&file.path) else {
                continue;
            };
            let Some(manifest) = manifests
                .entry(pubspec.clone())
                .or_insert_with(|| read_manifest(&pubspec))
            else {
                continue;
            };
            let is_library = is_library_file(&file.path, &pubspec);
            let imports = file.program().imports().flat_map(|directive| {
                std::iter::once(directive.uri()).chain(
                    directive
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                )
            });
            let exports = file.program().exports().flat_map(|directive| {
                std::iter::once(directive.uri()).chain(
                    directive
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                )
            });
            for uri in imports.chain(exports) {
                let value = uri.value();
                let Some(package) = imported_package(&value) else {
                    continue;
                };
                if package == manifest.name
                    || package == "flutter_gen"
                    || manifest.dependencies.contains(package)
                    || (!is_library && manifest.dev_dependencies.contains(package))
                {
                    continue;
                }
                diagnostics.push(Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!(
                        "Declare `{package}` in pubspec {} before importing it.",
                        if is_library {
                            "dependencies"
                        } else {
                            "dependencies or dev_dependencies"
                        }
                    ),
                    file.path.to_string_lossy().into_owned(),
                    uri.span(),
                ));
            }
        }
        diagnostics
    }
}
