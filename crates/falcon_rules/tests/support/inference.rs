//! SDK-free semantic test context built from parsed Dart interface declarations.

use std::fs;
use std::path::Path;

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, ExternalLibraries, ExternalSources, IdentityIndex,
    IdentitySource, LibrarySource, Rule, RuleAnalysis, SignatureIndex, TypeIndex,
    canonical_or_lexical,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;

#[path = "sdk_fixture.rs"]
pub mod sdk_fixture;
pub use sdk_fixture::sdk_project;

// This support module is also included by binaries that select fixtures directly.
#[allow(dead_code)]
pub fn requires_sdk(file: &Path) -> bool {
    sdk_fixture::requires_sdk(file)
}

// Semantic analysis uses staged SDK paths, but user globs select original source
// files. Freeze that selection before changing the file's filesystem identity.
fn config_for_source_file<'a>(
    config: &'a FalconConfig,
    file: &Path,
) -> std::borrow::Cow<'a, FalconConfig> {
    if config.overrides.is_empty() {
        return std::borrow::Cow::Borrowed(config);
    }
    let mut config = config.clone();
    let path = file.to_string_lossy();
    config.overrides.retain(|entry| entry.matches(&path));
    for entry in &mut config.overrides {
        entry.includes = falcon_config::PathFilter::default();
    }
    std::borrow::Cow::Owned(config)
}

pub fn analyze(
    rule: &dyn Rule,
    file: &Path,
    source: &str,
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    let dir = if file.is_file() {
        sdk_fixture::stage_project(sdk_fixture::source_directory(file))
    } else {
        sdk_project()
    };
    let root = dir.path();
    let app = if file.is_file() {
        let directory = sdk_fixture::source_directory(file);
        sdk_fixture::staged_file(root, directory, file)
    } else {
        root.join("app/lib").join(file.file_name().unwrap())
    };
    fs::create_dir_all(app.parent().unwrap()).unwrap();
    fs::write(&app, source).unwrap();
    let app = canonical_or_lexical(&app);
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |config| {
        let root = config?.flutter_root.as_ref()?;
        falcon_analyze::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
    });
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), program, source)], |path| {
            path == app
        });
    assert_eq!(
        environment.sdk(&app).unwrap().root,
        canonical_or_lexical(&root.join("flutter/bin/cache/dart-sdk")),
    );
    assert!(!externals.libraries().is_empty());
    for library in externals.libraries() {
        assert!(!library.has_unresolved_parts);
        for file in library.files.iter() {
            assert!(!file.has_parse_errors, "{}", file.path.display());
        }
    }
    let sources = [IdentitySource {
        path: &app,
        parsed: &parsed,
        has_parse_errors: !errors.is_empty(),
    }];
    let identities = IdentityIndex::build(
        &sources,
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            app.as_path(),
            LibrarySource {
                program,
                has_parse_errors: !errors.is_empty(),
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), parsed.clone())],
        Some(&externals),
        &identities,
        &types,
    );
    let config = config_for_source_file(config, file);
    let context = AnalyzeContext::new(&app, &config, &parsed)
        .with_types(&types)
        .with_identities(&identities)
        .with_signatures(&signatures)
        .with_environment(Some(&environment));
    rule.analyze(program, &context)
}

// This support module is included by binaries that do not all use marker assertions.
#[allow(dead_code)]
pub fn assert_diagnostics(source: &str, rule_name: &str, diagnostics: &[Diagnostic], count: usize) {
    let marker = format!("/* expect: {rule_name} */");
    let expected: Vec<_> = source
        .lines()
        .enumerate()
        .filter_map(|(line, text)| text.contains(&marker).then_some(line + 1))
        .collect();
    assert_eq!(expected.len(), count);
    let mut actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.rule, rule_name);
            source[..diagnostic.span.start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1
        })
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, expected, "{diagnostics:?}");
}

#[allow(dead_code)]
pub fn assert_lints(rule_name: &str, source: &str, count: usize) {
    let falcon_dart_parser::ParseResult { errors, .. } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let rules = crate::all_rules();
    let rule = &rules.iter().find(|(name, _)| *name == rule_name).unwrap().1;
    let diagnostics = analyze(
        rule.as_ref(),
        Path::new("main.dart"),
        source,
        &FalconConfig::default(),
    );
    assert_diagnostics(source, rule_name, &diagnostics, count);
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn staged_paths_preserve_original_rooted_override_selection_and_order() {
        let root = std::env::temp_dir().join("falcon-sdk-original-config-root");
        let mut original: FalconConfig = serde_json::from_value(json!({
            "overrides": [
                { "includes": ["**/bad.dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": true }
                    }
                } } } },
                { "includes": ["**/bad.dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": false }
                    }
                } } } },
                { "includes": ["**/other.dart"], "linter": { "rules": { "performance": {
                    "move-variable-closer-to-its-usage": {
                        "level": "warn", "options": { "ignore_after_if": true }
                    }
                } } } }
            ]
        }))
        .unwrap();
        original.resolve_paths(&root);
        let staged_path = std::env::temp_dir().join("falcon-sdk-staged/app/lib/bad.dart");
        let projected = config_for_source_file(&original, &root.join("bad.dart"));
        assert_eq!(projected.overrides.len(), 2);
        assert_eq!(
            projected.rule_options_for(
                &staged_path.to_string_lossy(),
                "performance",
                "move-variable-closer-to-its-usage",
            ),
            Some(&json!({ "ignore_after_if": false })),
        );
        assert!(
            original
                .rule_options_for(
                    &staged_path.to_string_lossy(),
                    "performance",
                    "move-variable-closer-to-its-usage",
                )
                .is_none()
        );
        assert_eq!(original.overrides.len(), 3);
    }
}
