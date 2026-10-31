//! Require package names to be valid `lower_case_with_underscores` identifiers.

use crate::lint::naming::is_snake_case;
use std::path::Path;

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use serde_yaml::Value;

use super::yaml_string_span;

#[derive(Default)]
pub struct PackageNames;

impl CrossFileRule for PackageNames {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut seen = std::collections::HashSet::new();
        files
            .iter()
            .filter_map(|file| environment.package(file.path()))
            .filter(|package| seen.insert(&package.manifest))
            .filter_map(|package| match package.snapshot() {
                falcon_analyze::ManifestSnapshot::Valid { source, yaml } => {
                    check_pubspec(&package.manifest, source, yaml)
                }
                falcon_analyze::ManifestSnapshot::Missing
                | falcon_analyze::ManifestSnapshot::Malformed(_)
                | falcon_analyze::ManifestSnapshot::Inaccessible(_) => None,
            })
            .collect()
    }
}

fn check_pubspec(path: &Path, source: &str, root: &Value) -> Option<Diagnostic> {
    let name_value = root.as_mapping()?.get(Value::String("name".to_string()))?;
    let name = name_value.as_str()?;
    if is_valid_package_name(name) {
        return None;
    }
    let span = yaml_string_span(source, root, name_value)?;
    let mut diagnostic = Diagnostic::new(
        "",
        Severity::Warning,
        format!("The package name `{name}` isn't a `lower_case_with_underscores` identifier."),
        path.to_string_lossy().into_owned(),
        span,
    );
    diagnostic.resolve_position(source);
    Some(diagnostic)
}

fn is_valid_package_name(name: &str) -> bool {
    let identifier = name.trim_start_matches('_');
    is_snake_case(identifier)
        && !identifier.ends_with('_')
        && !identifier.contains("__")
        && !is_reserved_word(name)
}

fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "else"
            | "enum"
            | "extends"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "if"
            | "in"
            | "is"
            | "new"
            | "null"
            | "rethrow"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_manifest_preserves_exact_package_name_diagnostic() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        let source = "\u{feff}name: BadName\n";
        std::fs::write(&manifest, source).unwrap();
        let package = falcon_analyze::PackageMetadata::load(&manifest);
        let falcon_analyze::ManifestSnapshot::Valid { source, yaml } = package.snapshot() else {
            panic!("BOM-prefixed YAML must be valid");
        };
        assert_eq!(package.identity.name, "BadName");
        assert_eq!(package.parsed_name(), Some("BadName"));
        let mut expected = Diagnostic::new(
            "",
            Severity::Warning,
            "The package name `BadName` isn't a `lower_case_with_underscores` identifier.",
            manifest.to_string_lossy(),
            falcon_syntax::cst::Span::new(9, 16),
        );
        expected.resolve_position(source);
        assert_eq!(
            serde_json::to_value(check_pubspec(&package.manifest, source, yaml)).unwrap(),
            serde_json::to_value(Some(expected)).unwrap(),
        );
    }

    #[test]
    fn validates_identifier_shape_and_reserved_words() {
        for valid in [
            "falcon",
            "dart_falcon",
            "_private",
            "___private",
            "package2",
        ] {
            assert!(is_valid_package_name(valid), "{valid}");
        }
        for invalid in [
            "",
            "2package",
            "DartFalcon",
            "dart-falcon",
            "class",
            "$pkg",
            "_",
            "___",
            "bad__name",
            "__bad__name",
            "bad_name_",
        ] {
            assert!(!is_valid_package_name(invalid), "{invalid}");
        }
    }
}
