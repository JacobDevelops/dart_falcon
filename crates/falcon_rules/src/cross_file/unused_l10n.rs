//! Report template ARB message keys with no identifier reference in analyzed Dart sources.
//!
//! Reads Flutter `gen-l10n` paths from `l10n.yaml`, or defaults to
//! `lib/l10n/app_en.arb` when no configuration exists. Diagnostics point to the
//! template key; metadata keys starting with `@` are ignored. The configured
//! output localization file and its locale siblings in the output directory do
//! not establish usage. Locale suffixes must occur in an ARB filename or its
//! `@@locale` metadata; unrecognized siblings conservatively establish usage.
//! Any matching identifier-like Dart token outside those files counts, including
//! declarations, contextual keywords (even in keyword positions), getters, calls,
//! tear-offs and interpolation.
//! Receiver types and the configured output class are deliberately not required,
//! since extension accessors can hide them. References in other analyzed packages
//! count too, supporting shared localization packages and example applications.
//! Missing inputs are silent; malformed or inaccessible localization metadata warns
//! once per failure episode and skips unused-localization analysis for that package.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};

#[derive(Default)]
pub struct UnusedL10n;

impl CrossFileRule for UnusedL10n {
    fn prepare_project(
        &self,
        environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let packages: BTreeSet<_> = files
            .iter()
            .filter_map(|file| environment.package(file.path()))
            .map(|package| &package.manifest)
            .collect();
        let templates: Vec<_> = packages
            .iter()
            .filter_map(|manifest| environment.localization(manifest))
            .collect();
        if templates.is_empty() {
            return Vec::new();
        }
        let mut used = HashSet::new();
        for file in files {
            if file
                .path()
                .extension()
                .is_none_or(|extension| extension != "dart")
            {
                continue;
            }
            let path = environment
                .normalized_path(file.path())
                .unwrap_or(file.path());
            if templates
                .iter()
                .any(|template| is_generated(path, &template.output_file, &template.locales))
            {
                continue;
            }
            used.extend(
                file.tree()
                    .tokens(file.source())
                    .filter(|token| token.kind().is_identifier_like())
                    .map(falcon_syntax::cst::SyntaxToken::text),
            );
        }
        let mut diagnostics = Vec::new();
        for template in templates {
            for (key, span) in template_keys(&template.source) {
                if key.starts_with('@') || used.contains(key.as_ref()) {
                    continue;
                }
                let mut diagnostic = Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!("Localization key `{key}` is never used."),
                    template.path.to_string_lossy().into_owned(),
                    span,
                );
                diagnostic.resolve_position(&template.source);
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }
}

fn is_generated(path: &Path, output: &Path, locales: &HashSet<String>) -> bool {
    if path == output {
        return true;
    }
    if path.parent() != output.parent() {
        return false;
    }
    let Some(stem) = output.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    path.file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(stem))
        .and_then(|suffix| suffix.strip_prefix('_'))
        .is_some_and(|locale| locales.contains(locale))
}

fn template_keys(source: &str) -> impl Iterator<Item = (Cow<'_, str>, Span)> {
    let bytes = source.as_bytes();
    let mut depth = 0;
    let mut offset = 0;
    std::iter::from_fn(move || {
        while offset < bytes.len() {
            match bytes[offset] {
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                b'"' => {
                    let start = offset;
                    offset += 1;
                    while offset < bytes.len() {
                        match bytes[offset] {
                            b'\\' => offset += 2,
                            b'"' => break,
                            _ => offset += 1,
                        }
                    }
                    let end = offset + 1;
                    if depth == 1
                        && bytes[end..].iter().find(|byte| !byte.is_ascii_whitespace())
                            == Some(&b':')
                    {
                        offset = end;
                        let spelling = &source[start + 1..end - 1];
                        let key = if spelling.contains('\\') {
                            Cow::Owned(serde_json::from_str::<String>(&source[start..end]).ok()?)
                        } else {
                            Cow::Borrowed(spelling)
                        };
                        return Some((key, Span { start, end }));
                    }
                }
                _ => {}
            }
            offset += 1;
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::AnalysisEnvironment;
    use std::sync::Arc;

    fn files(root: &Path) -> Vec<ProjectFile> {
        [
            ("main.dart", "void main() { print(used); }"),
            (
                "generated/localizations_fr.dart",
                "String get unused => '';",
            ),
        ]
        .into_iter()
        .map(|(path, source)| {
            ProjectFile::new(
                root.join(path),
                Arc::new(falcon_analyze::ParsedFile::parse(source)),
            )
            .unwrap()
        })
        .collect()
    }

    fn fixture(root: &Path) {
        std::fs::create_dir_all(root.join("messages")).unwrap();
        std::fs::write(root.join("pubspec.yaml"), "name: app\n").unwrap();
        std::fs::write(root.join("l10n.yaml"), "arb-dir: messages\noutput-dir: generated\noutput-localization-file: localizations.dart\n").unwrap();
        std::fs::write(
            root.join("messages/app_en.arb"),
            "{\"used\":\"yes\",\"unused\":\"no\"}",
        )
        .unwrap();
        std::fs::write(root.join("messages/app_fr.arb"), "{\"@@locale\":\"fr\"}").unwrap();
    }

    fn capture(files: &[ProjectFile]) -> AnalysisEnvironment {
        let mut environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        environment.capture_localization(files.iter().map(ProjectFile::path));
        environment
    }

    #[test]
    fn localization_consumes_captured_facts_after_all_metadata_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let files = files(root.path());
        let environment = capture(&files);
        let config = FalconConfig::default();
        let before = UnusedL10n.prepare_project(&environment, &files, &config, None);
        assert_eq!(before.len(), 1);
        assert!(before[0].message.contains("`unused`"));
        std::fs::remove_file(root.path().join("pubspec.yaml")).unwrap();
        std::fs::remove_file(root.path().join("l10n.yaml")).unwrap();
        std::fs::remove_dir_all(root.path().join("messages")).unwrap();
        assert_eq!(
            serde_json::to_value(UnusedL10n.prepare_project(&environment, &files, &config, None))
                .unwrap(),
            serde_json::to_value(before).unwrap(),
        );
    }

    #[test]
    fn malformed_and_inaccessible_localization_inputs_warn_once_and_skip() {
        for relative in ["l10n.yaml", "messages/app_en.arb", "messages/app_fr.arb"] {
            for bytes in [b"[".as_slice(), &[0xff]] {
                let root = tempfile::tempdir().unwrap();
                fixture(root.path());
                let path = root.path().join(relative);
                std::fs::write(&path, bytes).unwrap();
                let files = files(root.path());
                let environment = capture(&files);
                for _ in 0..2 {
                    assert!(
                        UnusedL10n
                            .prepare_project(&environment, &files, &FalconConfig::default(), None)
                            .is_empty()
                    );
                }
                let warnings = environment.take_warnings();
                assert_eq!(warnings.len(), 1, "{relative}: {warnings:?}");
                assert!(warnings[0].contains(path.to_str().unwrap()));
                assert_eq!(environment.take_warnings(), Vec::<String>::new());
            }
        }
    }

    #[test]
    fn unreadable_localization_inputs_warn_with_the_failed_path() {
        for relative in ["l10n.yaml", "messages/app_en.arb", "messages/app_fr.arb"] {
            let root = tempfile::tempdir().unwrap();
            fixture(root.path());
            let path = root.path().join(relative);
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            let files = files(root.path());
            let environment = capture(&files);
            assert!(
                UnusedL10n
                    .prepare_project(&environment, &files, &FalconConfig::default(), None)
                    .is_empty()
            );
            let warnings = environment.take_warnings();
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains(path.to_str().unwrap()));
        }
    }

    #[test]
    fn oversized_localization_inputs_are_bounded_and_recovery_rearms_warning() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let files = files(root.path());
        let path = root.path().join("messages/app_fr.arb");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(falcon_config::MAX_METADATA_BYTES + 1).unwrap();
        let mut environment = capture(&files);
        assert!(
            UnusedL10n
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .is_empty()
        );
        assert_eq!(environment.take_warnings().len(), 1);
        environment.refresh_localization(std::slice::from_ref(&path));
        assert!(
            UnusedL10n
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .is_empty()
        );
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
        std::fs::write(&path, "{}").unwrap();
        environment.refresh_localization(std::slice::from_ref(&path));
        assert_eq!(
            UnusedL10n
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .len(),
            1
        );
        std::fs::write(&path, [0xff]).unwrap();
        environment.refresh_localization(std::slice::from_ref(&path));
        assert!(
            UnusedL10n
                .prepare_project(&environment, &files, &FalconConfig::default(), None)
                .is_empty()
        );
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[test]
    fn absent_localization_paths_and_non_directory_ancestors_are_silent() {
        for settings in [
            "arb-dir: missing\n",
            "arb-dir: main.dart/child\n",
            "arb-dir: messages\ntemplate-arb-file: app_en.arb/child\n",
        ] {
            let root = tempfile::tempdir().unwrap();
            fixture(root.path());
            std::fs::write(root.path().join("main.dart"), "").unwrap();
            std::fs::write(root.path().join("l10n.yaml"), settings).unwrap();
            let files = files(root.path());
            let environment = capture(&files);
            assert!(
                UnusedL10n
                    .prepare_project(&environment, &files, &FalconConfig::default(), None)
                    .is_empty()
            );
            assert_eq!(environment.take_warnings(), Vec::<String>::new());
        }
    }

    #[test]
    fn discovery_does_not_capture_disabled_localization_metadata() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        std::fs::write(root.path().join("l10n.yaml"), "[").unwrap();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        assert!(environment.localization_watch_files().next().is_none());
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }

    #[test]
    fn localization_refresh_keeps_unaffected_package_facts() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fixture(first.path());
        fixture(second.path());
        let first_files = files(first.path());
        let second_files = files(second.path());
        let mut environment = capture(&first_files);
        environment.extend(second_files.iter().map(ProjectFile::path));
        environment.capture_localization(second_files.iter().map(ProjectFile::path));
        let config = FalconConfig::default();
        let before = UnusedL10n.prepare_project(&environment, &second_files, &config, None);
        std::fs::write(second.path().join("messages/app_en.arb"), "[").unwrap();
        environment.refresh_localization(&[first.path().join("messages/app_en.arb")]);
        assert_eq!(
            serde_json::to_value(UnusedL10n.prepare_project(
                &environment,
                &second_files,
                &config,
                None
            ))
            .unwrap(),
            serde_json::to_value(before).unwrap(),
        );
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }
}
