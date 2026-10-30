//! Warns when package implementation files import their own public entrypoint exports.
//!
//! Import the implementation directly instead of depending on a public barrel that
//! can introduce import cycles. Only files under `lib/src/` are checked by default;
//! set `only-in-src` to false to check other files in the same package as well.
//! An entrypoint is a file under `lib/`, outside `lib/src/`, with at least one
//! export directive, including when the entrypoint is outside the analyzed set.

use std::collections::HashMap;

use falcon_analyze::{AnalysisEnvironment, CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic, Severity};
use falcon_syntax::cst::Span;

use super::{ResolvedRef, resolve_directive_uri};
use crate::lint::options;

#[derive(Default)]
pub struct AvoidImportingEntrypointExports;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Restrict importing files to the package's `lib/src/` directory.
    only_in_src: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { only_in_src: true }
    }
}
options::impl_options!(Options);

impl CrossFileRule for AvoidImportingEntrypointExports {
    fn prepare_project(
        &self,
        environment: &AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut barrels: HashMap<_, _> = files
            .iter()
            .filter(|file| file.program().exports().next().is_some())
            .map(|file| (super::canonical_package_path(&file.path), true))
            .collect();
        let mut analyzed_seeded = false;
        let mut diagnostics = Vec::new();
        for file in files {
            let path = file.path.to_string_lossy();
            let settings = match options::cross_file::<Options>(config, &path, Self::NAME) {
                Ok(settings) => settings,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(
                        CONFIG_DIAGNOSTIC_ID,
                        Severity::Error,
                        error.to_string(),
                        path.as_ref(),
                        Span::new(0, 0),
                    ));
                    continue;
                }
            };
            let file_package = environment.file_package(&file.path);
            let Some(package) = file_package.identity() else {
                continue;
            };
            let source = super::canonical_package_path(&file.path);
            if settings.only_in_src
                && !source
                    .strip_prefix(&package.lib_root)
                    .is_ok_and(|relative| relative.starts_with("src"))
            {
                continue;
            }
            for import in file.program().imports() {
                for uri in std::iter::once(import.uri()).chain(
                    import
                        .configurable_uris()
                        .map(falcon_syntax::cst::ConfigurableUri::uri),
                ) {
                    let value = uri.value();
                    let Some(ResolvedRef::Path(target)) = resolve_directive_uri(
                        &source,
                        &value,
                        Some(package),
                        std::slice::from_ref(package),
                    ) else {
                        continue;
                    };
                    let Ok(relative) = target.strip_prefix(&package.lib_root) else {
                        continue;
                    };
                    if relative.starts_with("src") || source == target {
                        continue;
                    }
                    if !analyzed_seeded && !barrels.contains_key(&target) {
                        analyzed_seeded = true;
                        for file in files {
                            barrels
                                .entry(super::canonical_package_path(&file.path))
                                .or_insert(false);
                        }
                    }
                    let exports = *barrels
                        .entry(target)
                        .or_insert_with_key(|target| has_exports(target));
                    if !exports {
                        continue;
                    }
                    diagnostics.push(Diagnostic::new(
                        "",
                        Severity::Warning,
                        "Avoid importing entrypoint exports; import the source file directly instead.",
                        path.as_ref(),
                        uri.span(),
                    ));
                }
            }
        }
        diagnostics
    }
}

fn has_exports(path: &std::path::Path) -> bool {
    let Ok(source) = falcon_analyze::read_source(path) else {
        return false;
    };
    let parsed = falcon_dart_parser::parser::parse(&source);
    parsed.tree.program(&source).exports().next().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_entrypoints_outside_analyzed_files() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/corpus/avoid-importing-entrypoint-exports/cross-file/bad/lib/src/consumer.dart",
        );
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = falcon_dart_parser::parser::parse(&source);
        let files = [ProjectFile {
            path,
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into_boxed_str(),
                tree: parsed.tree,
            }),
            has_parse_errors: false,
        }];
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(|file| file.path.as_path()),
            |_| None,
        );
        let diagnostics = AvoidImportingEntrypointExports.prepare_project(
            &environment,
            &files,
            &FalconConfig::default(),
            None,
        );
        assert_eq!(diagnostics.len(), 5);
    }

    #[test]
    fn analyzed_sources_override_entrypoints_on_disk() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/avoid-importing-entrypoint-exports/cross-file/bad/lib");
        let files: Vec<_> = [
            (root.join("src/consumer.dart"), "import '../api.dart';"),
            (root.join("src/../api.dart"), ""),
        ]
        .into_iter()
        .map(|(path, source)| {
            let parsed = falcon_dart_parser::parser::parse(source);
            ProjectFile {
                path,
                parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                    source: source.into(),
                    tree: parsed.tree,
                }),
                has_parse_errors: false,
            }
        })
        .collect();
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(|file| file.path.as_path()),
            |_| None,
        );
        assert!(
            AvoidImportingEntrypointExports
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .is_empty()
        );
    }
}
