//! Report literal asset paths missing from the package's Flutter asset declarations.
//!
//! Only `lib/` sources in packages with a `flutter:` section are checked.
//! Literal `assets/` paths with a filename extension must be declared as assets,
//! fonts, shaders or deferred-component assets, including direct directory children.
//! Literal adjacent strings are concatenated. Dynamic strings, directive URIs,
//! `packages/` paths and arguments to calls with `package:` are exempt. Disk
//! existence is not checked; resolution variants are not independent declarations.

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::flutter_assets::{
    AssetEntry, asset_packages, asset_strings, has_package_argument, literal_value,
};
use crate::lint::support::package_manifests::is_library_file;

#[derive(Default)]
pub struct MissingAssets;

impl CrossFileRule for MissingAssets {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for package in asset_packages(files, false) {
            for file in package
                .files
                .iter()
                .filter(|file| is_library_file(&file.path, &package.pubspec))
            {
                for literal in asset_strings(file).filter(|literal| !has_package_argument(*literal))
                {
                    let Some(value) = literal_value(literal) else {
                        continue;
                    };
                    if !looks_like_asset_file(&value)
                        || package
                            .declarations
                            .iter()
                            .any(|path| AssetEntry::path_covers(path, &value))
                    {
                        continue;
                    }
                    let mut diagnostic = Diagnostic::new(
                        "",
                        Severity::Warning,
                        format!(
                            "Asset `{value}` is not declared under `flutter: assets:` in `pubspec.yaml`."
                        ),
                        file.path.to_string_lossy().into_owned(),
                        literal.span(),
                    );
                    diagnostic.resolve_position(&file.parsed.source);
                    diagnostics.push(diagnostic);
                }
            }
        }
        diagnostics
    }
}

fn looks_like_asset_file(value: &str) -> bool {
    value.starts_with("assets/")
        && value.rsplit('/').next().is_some_and(|name| {
            name.rsplit_once('.')
                .is_some_and(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        })
}
