//! Require `pubspec.yaml` dependency sections to be sorted alphabetically.
//!
//! Adopted from the official `sort_pub_dependencies` lint. The `dependencies`,
//! `dev_dependencies`, and `dependency_overrides` mappings of every package
//! `pubspec.yaml` enclosing an analyzed file are walked in source order, and
//! the first package name that sorts before its predecessor is reported; like
//! the official rule, each section reports at most one name (the first break in
//! the order) so that fixing it and re-running converges. Names compare by
//! UTF-16 code units, exactly as Dart's `String.compareTo`, so `Zebra` sorts
//! before `apple`. Malformed YAML (including duplicate keys) and non-mapping
//! sections are skipped. This is a cross-file rule because it reads the package
//! manifest rather than Dart source.

use std::cmp::Ordering;
use std::path::Path;

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use marked_yaml::{LoaderOptions, parse_yaml_with_options};

use super::{character_to_byte, project_pubspecs, scalar_source_span};

#[derive(Default)]
pub struct SortPubDependencies;

const NAME: &str = "sort-pub-dependencies";
const SECTIONS: [&str; 3] = ["dependencies", "dev_dependencies", "dependency_overrides"];

impl CrossFileRule for SortPubDependencies {
    fn analyze_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        project_pubspecs(files)
            .into_iter()
            .flat_map(|path| check_pubspec(&path))
            .collect()
    }
}

fn check_pubspec(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    // Reject invalid mappings during the same location-preserving parse.
    let options = LoaderOptions::default().error_on_duplicate_keys(true);
    let Ok(root) = parse_yaml_with_options(0, &source, options) else {
        return Vec::new();
    };
    let Some(root) = root.as_mapping() else {
        return Vec::new();
    };
    SECTIONS
        .iter()
        .filter_map(|section| {
            let dependencies = root.get_mapping(section)?;
            let mut names: Vec<(usize, &str)> = dependencies
                .keys()
                .filter_map(|key| Some((key.span().start()?.character(), key.as_str())))
                .collect();
            names.sort_unstable_by_key(|(character, _)| *character);
            let (character, name) = first_unsorted(&names)?;
            let start = character_to_byte(&source, character)?;
            let span = scalar_source_span(&source, start)?;
            let mut diagnostic = Diagnostic::new(
                NAME,
                Severity::Warning,
                format!("Dependencies not sorted alphabetically: '{name}' is out of order."),
                path.to_string_lossy().into_owned(),
                span,
            );
            diagnostic.resolve_position(&source);
            Some(diagnostic)
        })
        .collect()
}

/// The first `(position, name)` whose name sorts before the previous name.
fn first_unsorted<'a>(names: &[(usize, &'a str)]) -> Option<(usize, &'a str)> {
    names
        .windows(2)
        .find(|pair| dart_compare(pair[1].1, pair[0].1) == Ordering::Less)
        .map(|pair| pair[1])
}

/// Dart `String.compareTo`: lexicographic over UTF-16 code units.
fn dart_compare(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_the_first_break_in_order() {
        let names = [(0, "b"), (1, "a"), (2, "c"), (3, "aa")];
        assert_eq!(first_unsorted(&names), Some((1, "a")));
        assert_eq!(first_unsorted(&[(0, "a"), (1, "b")]), None);
    }

    #[test]
    fn compares_like_dart_string_compare_to() {
        assert_eq!(dart_compare("Zebra", "apple"), Ordering::Less);
        assert_eq!(dart_compare("flutter", "flutter_test"), Ordering::Less);
        assert_eq!(dart_compare("a_b", "ab"), Ordering::Less);
    }

    #[test]
    fn skips_duplicate_keys_even_outside_dependency_sections() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pubspec.yaml");
        std::fs::write(
            &path,
            "name: example\nmetadata:\n  key: first\n  key: second\ndependencies:\n  zebra: any\n  apple: any\n",
        )
        .unwrap();
        assert!(check_pubspec(&path).is_empty());
    }
}
