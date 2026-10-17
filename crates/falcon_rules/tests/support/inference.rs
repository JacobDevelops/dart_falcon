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
    let falcon_dart_parser::ParseResult {
        program,
        errors,
        syntax,
    } = parse(source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |config| {
        let root = config?.flutter_root.as_ref()?;
        falcon_analyze::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
    });
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
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
        program: &program,
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
                program: &program,
                has_parse_errors: !errors.is_empty(),
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let context = AnalyzeContext::new(&app, source, config, &syntax)
        .with_types(&types)
        .with_identities(&identities)
        .with_signatures(&signatures)
        .with_environment(Some(&environment));
    rule.analyze(&program, &context)
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
