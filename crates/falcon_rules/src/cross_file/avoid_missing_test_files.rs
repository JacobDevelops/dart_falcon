//! Requires configured source files to have a corresponding test file.
//!
//! `patterns` is an empty-by-default list of regular expressions matched against
//! package-relative paths (including `lib/`, with `/` separators). Tests mirror the
//! source path: `lib/src/value.dart` requires `test/src/value_test.dart`. A
//! configured file is reported even when the package has no test directory.

use falcon_analyze::{AnalysisEnvironment, CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic, Severity};
use falcon_syntax::cst::Span;

use crate::lint::options::{self, Regex};

#[derive(Default)]
pub struct AvoidMissingTestFiles;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Regular expressions selecting package-relative source paths requiring tests.
    patterns: Vec<Regex>,
}
options::impl_options!(Options);

impl CrossFileRule for AvoidMissingTestFiles {
    fn prepare_project(
        &self,
        environment: &AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
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
            if settings.patterns.is_empty() {
                continue;
            }
            let file_package = environment.file_package(&file.path);
            let Some(package) = file_package.identity() else {
                continue;
            };
            let source = super::canonical_package_path(&file.path);
            let Ok(relative) = source.strip_prefix(&package.lib_root) else {
                continue;
            };
            let package_relative = format!("lib/{}", relative.to_string_lossy().replace('\\', "/"));
            if !settings
                .patterns
                .iter()
                .any(|pattern| pattern.is_match(&package_relative))
            {
                continue;
            }
            let Some(root) = file_package
                .metadata()
                .and_then(|metadata| metadata.manifest.parent())
            else {
                continue;
            };
            let Some(stem) = relative.file_stem() else {
                continue;
            };
            let mut test = root.join("test").join(relative);
            test.set_file_name(format!("{}_test.dart", stem.to_string_lossy()));
            if test.is_file() {
                continue;
            }
            diagnostics.push(Diagnostic::new(
                "",
                Severity::Warning,
                format!(
                    "This file does not have a corresponding test file; add `{}`.",
                    test.strip_prefix(root)
                        .unwrap_or(&test)
                        .to_string_lossy()
                        .replace('\\', "/")
                ),
                path.as_ref(),
                Span::new(0, 0),
            ));
        }
        diagnostics
    }
}
