//! Shared identity contexts and SDK staging for rule unit tests.

use falcon_analyze::{
    AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, ParsedFile, Rule, RuleAnalysis,
    SemanticModel, SignatureIndex, TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use std::path::Path;
use std::sync::Arc;

pub fn identities(files: &[(&Path, &Arc<ParsedFile>)]) -> IdentityIndex {
    let sources: Vec<_> = files
        .iter()
        .map(|&(path, parsed)| IdentitySource {
            path,
            parsed,
            has_parse_errors: false,
        })
        .collect();
    IdentityIndex::from_project_files(&sources, &[])
}

pub struct TestIndexes {
    pub identities: IdentityIndex,
    pub types: Option<TypeIndex>,
    pub signatures: SignatureIndex,
}

impl TestIndexes {
    pub fn new(files: &[(&Path, &Arc<ParsedFile>)]) -> Self {
        let identities = identities(files);
        let types = TypeIndex::from_resolved_files(
            files.iter().map(|&(path, parsed)| {
                (
                    path,
                    LibrarySource {
                        program: parsed.program(),
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files: Vec<_> = files
            .iter()
            .map(|&(path, parsed)| (path.to_path_buf(), Arc::clone(parsed)))
            .collect();
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        Self {
            identities,
            types: Some(types),
            signatures,
        }
    }

    pub fn single(path: &Path, parsed: &Arc<ParsedFile>, with_types: bool) -> Self {
        let identities = identities(&[(path, parsed)]);
        let types = with_types.then(|| {
            TypeIndex::from_resolved_files(
                [(
                    path,
                    LibrarySource {
                        program: parsed.program(),
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )],
                &identities,
            )
        });
        let model = SemanticModel::new(path, parsed, &identities, types.as_ref());
        let signatures = SignatureIndex::from_program(parsed.program(), &model);
        Self {
            identities,
            types,
            signatures,
        }
    }

    pub fn context<'a>(
        &'a self,
        path: &'a Path,
        config: &'a FalconConfig,
        parsed: &'a Arc<ParsedFile>,
    ) -> AnalyzeContext<'a> {
        let mut context = AnalyzeContext::new(path, config, parsed)
            .with_identities(&self.identities)
            .with_signatures(&self.signatures);
        if let Some(types) = &self.types {
            context = context.with_types(types);
        }
        context
    }
}

pub fn count(name: &'static str, rule: &dyn Rule, source: &str, with_types: bool) -> usize {
    let falcon_dart_parser::ParseResult { tree, .. } = parse(source);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let config = FalconConfig::default();
    let path = Path::new("t.dart");
    let indexes = TestIndexes::single(path, &parsed, with_types);
    let context = indexes.context(path, &config, &parsed);
    rule.analyze(name, parsed.program(), &context).len()
}

pub use crate::inference::sdk_fixture::write_sdk;

pub fn analyze_flutter_rule(
    name: &'static str,
    source: &str,
    rule: &dyn Rule,
) -> Vec<falcon_diagnostics::Diagnostic> {
    use falcon_analyze::{
        AnalysisEnvironment, ExternalLibraries, ExternalSources, canonical_or_lexical,
    };
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let app = root.join("app");

    for path in [
        app.join(".dart_tool"),
        app.join("lib"),
        root.join("flutter/packages/flutter/lib"),
        root.join("meta/lib"),
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    write_sdk(
        root,
        &[(
            "core",
            include_str!("../../tests/support/sdk/lib/core/core.dart"),
        )],
    );
    std::fs::write(
        root.join("flutter/packages/flutter/lib/widgets.dart"),
        r"
abstract class Key { const Key.empty(); }
abstract class LocalKey extends Key { const LocalKey() : super.empty(); }
abstract class GlobalKey<T> extends Key { const GlobalKey() : super.empty(); }
abstract class Widget {
  const Widget({this.key});
  final Key? key;
}
abstract class StatelessWidget extends Widget { const StatelessWidget({super.key}); }
",
    )
    .unwrap();
    std::fs::write(
            root.join("meta/lib/meta.dart"),
            "class _Annotation { const _Annotation(); } const visibleForTesting = _Annotation(); const internal = _Annotation();",
        )
        .unwrap();
    std::fs::write(
        app.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"flutterRoot":"../../flutter","packages":[
                {"name":"app","rootUri":"../","packageUri":"lib/"},
                {"name":"flutter","rootUri":"../../flutter/packages/flutter","packageUri":"lib/"},
                {"name":"meta","rootUri":"../../meta","packageUri":"lib/"}
            ]}"#,
    )
    .unwrap();
    let path = app.join("lib/main.dart");
    std::fs::write(&path, source).unwrap();
    let falcon_dart_parser::ParseResult { errors, tree } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let environment = AnalysisEnvironment::discover([path.as_path()]);
    assert_eq!(
        environment.sdk(&path).unwrap().root,
        canonical_or_lexical(&root.join("flutter/bin/cache/dart-sdk")),
    );
    let external = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), program, source)],
        |candidate| candidate == path,
    );
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &external,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&external, &identities);
    let signatures = SignatureIndex::build(
        &[(path.clone(), std::sync::Arc::clone(&parsed))],
        Some(&external),
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(&path, &config, &parsed)
        .with_types(&types)
        .with_identities(&identities)
        .with_signatures(&signatures);
    rule.analyze(name, program, &context)
}
