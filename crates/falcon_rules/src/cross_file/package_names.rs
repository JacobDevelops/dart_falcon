//! Require package names to be valid `lower_case_with_underscores` identifiers.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use serde_yaml::Value;

use super::{enclosing_pubspec, yaml_string_span};

pub struct PackageNames;

const NAME: &str = "package-names";

impl CrossFileRule for PackageNames {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze_project(&self, files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
        project_pubspecs(files)
            .into_iter()
            .filter_map(|path| check_pubspec(&path))
            .collect()
    }
}

fn check_pubspec(path: &Path) -> Option<Diagnostic> {
    let source = std::fs::read_to_string(path).ok()?;
    let root: Value = serde_yaml::from_str(&source).ok()?;
    let name_value = root.as_mapping()?.get(Value::String("name".to_string()))?;
    let name = name_value.as_str()?;
    if is_valid_package_name(name) {
        return None;
    }
    let span = yaml_string_span(&source, &root, name_value)?;
    let mut diagnostic = Diagnostic::new(
        NAME,
        Severity::Warning,
        format!("The package name '{name}' isn't a lower_case_with_underscores identifier."),
        path.to_string_lossy().into_owned(),
        span,
    );
    diagnostic.resolve_position(&source);
    Some(diagnostic)
}

fn project_pubspecs(files: &[ProjectFile]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for file in files {
        if let Some(path) = enclosing_pubspec(&file.path)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }
    paths
}

fn is_valid_package_name(name: &str) -> bool {
    let identifier = name.trim_start_matches('_');
    let mut bytes = identifier.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
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
