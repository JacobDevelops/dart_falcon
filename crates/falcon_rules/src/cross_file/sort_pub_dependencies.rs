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

use super::{scalar_source_span, yaml_character_to_byte};

#[derive(Default)]
pub struct SortPubDependencies;

const SECTIONS: [&str; 3] = ["dependencies", "dev_dependencies", "dependency_overrides"];

impl CrossFileRule for SortPubDependencies {
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
            .flat_map(|package| match package.snapshot() {
                falcon_analyze::ManifestSnapshot::Valid { source, .. } => {
                    check_pubspec(&package.manifest, source)
                }
                falcon_analyze::ManifestSnapshot::Missing
                | falcon_analyze::ManifestSnapshot::Malformed(_)
                | falcon_analyze::ManifestSnapshot::Inaccessible(_) => Vec::new(),
            })
            .collect()
    }
}

fn check_pubspec(path: &Path, source: &str) -> Vec<Diagnostic> {
    let options = LoaderOptions::default();
    let Ok(root) = parse_yaml_with_options(
        0,
        source.strip_prefix('\u{feff}').unwrap_or(source),
        options,
    ) else {
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
            let start = yaml_character_to_byte(source, character)?;
            let span = scalar_source_span(source, start)?;
            let mut diagnostic = Diagnostic::new(
                "",
                Severity::Warning,
                format!("Dependencies not sorted alphabetically: `{name}` is out of order."),
                path.to_string_lossy().into_owned(),
                span,
            );
            diagnostic.resolve_position(source);
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
    fn duplicate_keys_make_the_snapshot_malformed_and_skip_sorting() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        let source = "name: example\nmetadata:\n  key: first\n  key: second\ndependencies:\n  zebra: any\n  apple: any\n";
        std::fs::write(&manifest, source).unwrap();
        let files = [ProjectFile::new(
            root.path().join("lib/main.dart"),
            std::sync::Arc::new(falcon_analyze::ParsedFile::parse("void main() {}")),
        )
        .unwrap()];
        let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        assert!(matches!(
            environment.package(files[0].path()).unwrap().snapshot(),
            falcon_analyze::ManifestSnapshot::Malformed(_)
        ));
        assert!(
            SortPubDependencies
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .is_empty()
        );
    }

    #[test]
    fn bom_manifest_preserves_exact_sorting_diagnostic() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        let files = [ProjectFile::new(
            root.path().join("lib/main.dart"),
            std::sync::Arc::new(falcon_analyze::ParsedFile::parse("void main() {}")),
        )
        .unwrap()];
        for (source, start) in [
            (
                "\u{feff}name: app\ndependencies:\n  zebra: any\n  apple: any\n",
                42,
            ),
            (
                "\u{feff}dependencies:\n  zebra: any\n  apple: any\nname: app\n",
                32,
            ),
        ] {
            std::fs::write(&manifest, source).unwrap();
            let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
                files.iter().map(ProjectFile::path),
                |_| None,
            );
            let falcon_analyze::ManifestSnapshot::Valid { source: captured, .. } =
                environment.package(files[0].path()).unwrap().snapshot()
            else {
                panic!("BOM-prefixed YAML must be valid");
            };
            assert_eq!(captured, source);
            assert_eq!(environment.take_warnings(), Vec::<String>::new());
            let mut expected = Diagnostic::new(
                "",
                Severity::Warning,
                "Dependencies not sorted alphabetically: `apple` is out of order.",
                manifest.to_string_lossy(),
                falcon_syntax::cst::Span::new(start, start + 5),
            );
            expected.resolve_position(source);
            assert_eq!(
                serde_json::to_value(SortPubDependencies.prepare_project(
                    &environment,
                    &files,
                    &FalconConfig::default(),
                    None,
                ))
                .unwrap(),
                serde_json::to_value([expected]).unwrap(),
            );
        }
    }
}
