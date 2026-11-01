//! Per-run source analysis: reparse edits without reloading the dependency graph.

use std::path::Path;
use std::sync::Arc;

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, ExternalIndexParts, ExternalLibraries, ExternalSources,
    ExternalTarget, IdentityIndex, IdentitySource, LibraryGrouping, PackageIdentity, ParsedFile,
    ProjectFile, SemanticIndexes, SignatureIndex, TypeIndex, build_semantic_indexes,
    canonical_or_lexical, library_unit_by, package_identities, rules_thread_diagnostic,
    source_size_diagnostic, syntax_error_diagnostics, update_semantic_indexes,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

use crate::{FixContext, ProjectAnalysis, analyze_document};

struct ExternalIndexes {
    identities: Arc<IdentityIndex>,
    types: Arc<TypeIndex>,
    signatures: Arc<SignatureIndex>,
}

struct ProjectOverlay {
    indexes: SemanticIndexes,
    grouping: LibraryGrouping,
    declaration_signature: u64,
    has_parse_errors: bool,
}

impl ExternalIndexes {
    fn borrowed(&self) -> ExternalIndexParts<'_> {
        ExternalIndexParts {
            identities: &self.identities,
            types: &self.types,
            signatures: &self.signatures,
        }
    }

    fn build(external: ExternalSources<'_>) -> Self {
        let identities = Arc::new(IdentityIndex::build_external(external));
        let mut types = TypeIndex::from_resolved_files(std::iter::empty(), &identities);
        types.add_external(external.libraries, &identities);
        let types = Arc::new(types);
        let signatures = Arc::new(SignatureIndex::build(
            &[],
            Some(external.libraries),
            &identities,
            &types,
        ));
        Self {
            identities,
            types,
            signatures,
        }
    }
}

/// The external graph and unopened library companions remain fixed for one run.
/// Declaration-preserving passes refresh only the requested file's body facts.
/// Declaration edits rebuild only affected libraries and their dependents.
pub(crate) struct AutofixAnalyzer<'a> {
    analysis: &'a ProjectAnalysis,
    config: &'a FalconConfig,
    environment: &'a AnalysisEnvironment,
    path: &'a Path,
    libraries: Option<ExternalLibraries>,
    initialized: bool,
    external_libraries: Option<&'a ExternalLibraries>,
    indexes: Option<ExternalIndexes>,
    companions: Vec<ProjectFile>,
    packages: Vec<PackageIdentity>,
    overlay: Option<ProjectOverlay>,
    pub(crate) source_failure: Option<falcon_analyze::SourceFailure>,
    #[cfg(test)]
    overlay_builds: usize,
    #[cfg(test)]
    declaration_library_rebuilds: usize,
    #[cfg(test)]
    body_refreshes: usize,
    #[cfg(test)]
    parser_failure_path: Option<std::path::PathBuf>,
}

impl<'a> AutofixAnalyzer<'a> {
    pub(crate) fn new(
        analysis: &'a ProjectAnalysis,
        config: &'a FalconConfig,
        environment: &'a mut AnalysisEnvironment,
        path: &'a Path,
        context: FixContext<'a>,
    ) -> Self {
        environment.extend(
            std::iter::once(path).chain(context.project_files.iter().map(ProjectFile::path)),
        );
        let target = canonical_or_lexical(path);
        let companions = context
            .project_files
            .iter()
            .filter(|file| canonical_or_lexical(file.path()) != target)
            .map(|file| {
                ProjectFile::new(file.path().to_path_buf(), Arc::clone(file.parsed()))
                    .expect("an existing project file has a parsed snapshot")
            })
            .collect();
        Self {
            analysis,
            config,
            environment,
            path,
            libraries: None,
            initialized: false,
            external_libraries: context.external_libraries,
            indexes: context.external.map(|parts| ExternalIndexes {
                identities: Arc::clone(parts.identities),
                types: Arc::clone(parts.types),
                signatures: Arc::clone(parts.signatures),
            }),
            companions,
            packages: Vec::new(),
            overlay: None,
            source_failure: None,
            #[cfg(test)]
            overlay_builds: 0,
            #[cfg(test)]
            declaration_library_rebuilds: 0,
            #[cfg(test)]
            body_refreshes: 0,
            #[cfg(test)]
            parser_failure_path: None,
        }
    }

    pub(crate) fn analyze(&mut self, source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = falcon_analyze::with_rules_stack(|| self.analyze_inner(source))
            .unwrap_or_else(|error| vec![rules_thread_diagnostic(self.path, &error)]);
        diagnostics.retain(|diagnostic| diagnostic.file_path == self.path.to_string_lossy());
        diagnostics
    }

    fn analyze_inner(&mut self, source: &str) -> Vec<Diagnostic> {
        let file = match self.parse(self.path, source) {
            Ok((file, diagnostics)) => (file, diagnostics),
            Err(diagnostic) => return vec![*diagnostic],
        };
        let (file, diagnostics) = file;
        if !self.analysis.resolve() {
            let ctx = AnalyzeContext::new(self.path, self.config, file.parsed())
                .with_environment(Some(self.environment));
            return analyze_document(self.analysis.registry(), self.config, &ctx, diagnostics);
        }
        if !self.initialized
            && let Err(error) = self.load_external(&file)
        {
            self.source_failure = Some(error);
            return Vec::new();
        }
        let declaration_signature = file.parsed().declaration_signature();
        if let Some(overlay) = self.overlay.as_mut().filter(|overlay| {
            !file.has_parse_errors()
                && !overlay.has_parse_errors
                && overlay.declaration_signature == declaration_signature
        }) {
            let id = overlay.indexes.identities.file(self.path).unwrap();
            overlay.indexes.refresh_file_body(id, file.parsed());
            #[cfg(test)]
            {
                self.body_refreshes += 1;
            }
        } else {
            let sources: Vec<_> = std::iter::once(&file)
                .chain(&self.companions)
                .map(|file| IdentitySource {
                    path: file.path(),
                    parsed: file.parsed(),
                    has_parse_errors: file.has_parse_errors(),
                })
                .collect();
            let external = self
                .external_libraries
                .or(self.libraries.as_ref())
                .map(|libraries| ExternalSources {
                    libraries,
                    environment: self.environment,
                });
            let cache = self.indexes.as_ref().map(ExternalIndexes::borrowed);
            if let Some(overlay) = self.overlay.as_mut() {
                // The environment, companions, and external graph are fixed for
                // this analyzer's lifetime, so its project indexes are reusable.
                let (grouping, rebuilt) = update_semantic_indexes(
                    &mut overlay.indexes,
                    &sources,
                    &self.packages,
                    external,
                    cache,
                );
                overlay.grouping = grouping;
                overlay.declaration_signature = declaration_signature;
                overlay.has_parse_errors = file.has_parse_errors();
                #[cfg(test)]
                {
                    self.declaration_library_rebuilds += rebuilt;
                }
                #[cfg(not(test))]
                let _ = rebuilt;
            } else {
                let (indexes, grouping) =
                    build_semantic_indexes(&sources, &self.packages, external, cache);
                self.overlay = Some(ProjectOverlay {
                    indexes,
                    grouping,
                    declaration_signature,
                    has_parse_errors: file.has_parse_errors(),
                });
                #[cfg(test)]
                {
                    self.overlay_builds += 1;
                }
            }
        }
        let overlay = self.overlay.as_ref().unwrap();
        let indexes = &overlay.indexes;
        let library = library_unit_by(&overlay.grouping, 0, |i| {
            if i == 0 {
                file.parsed()
            } else {
                self.companions[i - 1].parsed()
            }
        });
        let ctx = AnalyzeContext::new(self.path, self.config, file.parsed())
            .with_types(&indexes.types)
            .with_identities(&indexes.identities)
            .with_signatures(&indexes.signatures)
            .with_library(&library)
            .with_environment(Some(self.environment));
        analyze_document(self.analysis.registry(), self.config, &ctx, diagnostics)
    }

    fn parse(
        &self,
        path: &Path,
        source: &str,
    ) -> Result<(ProjectFile, Vec<Diagnostic>), Box<Diagnostic>> {
        if let Some(diagnostic) = source_size_diagnostic(path, source.len() as u64) {
            return Err(Box::new(diagnostic));
        }
        #[cfg(test)]
        if self.parser_failure_path.as_deref() == Some(path) {
            let error =
                falcon_analyze::with_rules_stack(|| panic!("autofix companion parser regression"))
                    .unwrap_err();
            return Err(Box::new(rules_thread_diagnostic(path, &error)));
        }
        let (parsed, errors) = ParsedFile::try_parse_for_package(
            source,
            self.environment.package_language_version(path),
        )
        .map_err(|(_, error)| Box::new(rules_thread_diagnostic(path, &error)))?;
        let diagnostics = syntax_error_diagnostics(path, &errors);
        Ok((
            ProjectFile::new(path.to_path_buf(), Arc::new(parsed))
                .expect("a successful parse produces a project snapshot"),
            diagnostics,
        ))
    }

    fn load_external(&mut self, file: &ProjectFile) -> Result<(), falcon_analyze::SourceFailure> {
        let path = canonical_or_lexical(self.path);
        let project_paths: std::collections::HashSet<_> = std::iter::once(path.clone())
            .chain(
                self.companions
                    .iter()
                    .map(|file| canonical_or_lexical(file.path())),
            )
            .collect();
        let roots: Vec<_> = std::iter::once(file)
            .chain(&self.companions)
            .map(|file| (file.path(), file.program(), file.source()))
            .collect();
        let owned = self.external_libraries.is_none().then(|| {
            ExternalLibraries::load(self.environment, &roots, |candidate| {
                project_paths.contains(candidate)
            })
        });
        let libraries = self.external_libraries.or(owned.as_ref()).unwrap();
        let roots =
            crate::required_source_roots(project_paths.iter().map(std::path::PathBuf::as_path));
        let required = |path: &Path| self.environment.project_source_required(path, &roots);
        if let Some(error) = libraries
            .worker_failure()
            .or_else(|| libraries.source_failure(required))
        {
            return Err(error);
        }
        for companion in libraries
            .project_companions()
            .filter(|candidate| owned.is_some() && !project_paths.contains(*candidate))
        {
            let source = match falcon_analyze::acquire_source(companion).companion(companion) {
                Ok(Some(source)) => source,
                Ok(None) => continue,
                Err(error) if required(companion) => return Err(error),
                Err(error) => {
                    tracing::warn!("{error}");
                    continue;
                }
            };
            let (file, _) = self.parse(companion, &source).map_err(|diagnostic| {
                falcon_analyze::SourceFailure {
                    path: companion.to_path_buf(),
                    source: Arc::new(std::io::Error::other(diagnostic.message)),
                }
            })?;
            self.companions.push(file);
        }
        self.packages = package_identities(
            std::iter::once(self.path).chain(self.companions.iter().map(ProjectFile::path)),
        );
        // An external library importing a project file must resolve against the
        // current overlay, rather than freezing that edge in the external layer.
        let has_project_dependency = libraries.libraries().iter().enumerate().any(|(instance, library)| {
            library.files.iter().any(|file| {
                file.program().imports().map(|import| import.uri().value())
                    .chain(file.program().exports().map(|export| export.uri().value()))
                    .chain(file.doc_imports.iter().map(|uri| std::borrow::Cow::Borrowed(uri.as_str())))
                    .any(|uri| matches!(
                        libraries.resolve_target(self.environment, &file.path, Some(instance), &uri),
                        Some(ExternalTarget::Path(target)) if project_paths.contains(&target)
                    ))
            })
        });
        if has_project_dependency {
            self.indexes = None;
        } else if self.indexes.is_none() {
            self.indexes = Some(ExternalIndexes::build(ExternalSources {
                libraries,
                environment: self.environment,
            }));
        }
        self.libraries = owned;
        self.initialized = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autofix_analysis_preserves_the_parent_and_subscriber() {
        const NAME: &str =
            "autofix_analysis::tests::autofix_analysis_preserves_the_parent_and_subscriber";
        use std::sync::Mutex;
        use tracing::{Subscriber, span::Attributes};
        use tracing_subscriber::{
            EnvFilter, Layer, layer::Context, prelude::*, registry::LookupSpan,
        };

        type Record = (&'static str, Option<&'static str>, std::thread::ThreadId);
        struct Capture(Arc<Mutex<Vec<Record>>>);
        impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
            fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
                let span = ctx.span(id).unwrap();
                self.0.lock().unwrap().push((
                    attrs.metadata().name(),
                    span.parent().map(|parent| parent.name()),
                    std::thread::current().id(),
                ));
            }
        }
        if std::env::var_os("FALCON_TRACING_TEST_CHILD").as_deref()
            != Some(std::ffi::OsStr::new(NAME))
        {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--nocapture"])
                .env("FALCON_TRACING_TEST_CHILD", NAME)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.lines().any(|line| line == "running 1 test"),
                "{NAME} must run exactly once successfully:\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let records = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new("off,falcon::stages=debug"))
            .with(Capture(records.clone()));
        let origin = std::thread::current().id();
        tracing::subscriber::with_default(subscriber, || {
            let _analysis = tracing::debug_span!(target: "falcon::stages", "analysis").entered();
            let config = FalconConfig::default();
            let analysis = ProjectAnalysis::new(&config);
            let outcome = crate::fix_file(
                &analysis,
                &config,
                &mut AnalysisEnvironment::default(),
                Path::new("main.dart"),
                "void main() {}",
                false,
            );
            assert!(outcome.stop.is_none());
        });
        let records = records.lock().unwrap();
        for name in [
            "semantic_index_build",
            "registry_preparation",
            "registry_reporting",
        ] {
            let stages: Vec<_> = records.iter().filter(|record| record.0 == name).collect();
            assert!(!stages.is_empty(), "missing {name}");
            assert!(
                stages
                    .iter()
                    .all(|record| record.1 == Some("analysis") && record.2 != origin)
            );
        }
    }

    #[test]
    fn body_fix_passes_do_not_reindex_companions() {
        let config = FalconConfig::default();
        let analysis =
            ProjectAnalysis::for_semantic_autofix_tests(falcon_rules::build_registry(&config));
        let files: Vec<_> = (0..64)
            .map(|i| {
                let source = format!("class Companion{i} {{}}");
                ProjectFile::new(
                    format!("/virtual/companion{i}.dart").into(),
                    Arc::new(ParsedFile::parse(source)),
                )
                .expect("companion fixture parses successfully")
            })
            .collect();
        let mut environment = AnalysisEnvironment::default();
        let libraries = ExternalLibraries::default();
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            Path::new("/virtual/main.dart"),
            FixContext {
                external_libraries: Some(&libraries),
                external: None,
                project_files: &files,
                cancelled: &|| false,
            },
        );
        for body in ["", "print(1);", "print('longer');", "print('x');"] {
            analyzer.analyze(&format!(
                "void main() {{ {body} }} String after(String value) => value;"
            ));
        }
        assert_eq!(analyzer.overlay_builds, 1);
        assert_eq!(analyzer.body_refreshes, 3);
        analyzer.analyze("void main() {} int after(int value) => value;");
        assert_eq!(analyzer.overlay_builds, 1);
        assert_eq!(analyzer.declaration_library_rebuilds, 1);
        assert_eq!(analyzer.body_refreshes, 3);
    }

    struct SemanticSnapshot;

    impl falcon_analyze::Rule for SemanticSnapshot {
        fn prepare<'a>(
            &self,
            _program: falcon_syntax::cst::Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> falcon_analyze::RulePreparation<'a> {
            let identities = ctx.identities().unwrap();
            let mut visible = Vec::with_capacity(10);
            for path in [ctx.file_path(), Path::new("/virtual/dependent.dart")] {
                let library = identities.library(identities.file(path).unwrap());
                for name in ["Target", "Changed", "Imported", "Alternate", "FromPart"] {
                    if identities
                        .resolve_declaration(
                            library,
                            falcon_analyze::QualifiedName { prefix: None, name },
                        )
                        .is_some()
                    {
                        visible.push(format!("{}:{name}", path.display()));
                    }
                }
            }
            falcon_analyze::RulePreparation::Diagnostics(vec![Diagnostic::new(
                "semantic-snapshot",
                falcon_diagnostics::Severity::Warning,
                visible.join(", "),
                ctx.file_path().to_string_lossy(),
                falcon_diagnostics::Span { start: 0, end: 0 },
            )])
        }
    }

    #[test]
    fn declaration_passes_update_dependents_without_rebuilding_unrelated_libraries() {
        let config = FalconConfig::default();
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("semantic-snapshot", Box::new(SemanticSnapshot));
        let analysis = ProjectAnalysis::for_semantic_autofix_tests(registry);
        let files: Vec<_> = [
            ("/virtual/dependent.dart", "import 'main.dart';"),
            ("/virtual/unrelated.dart", "class Unrelated {}"),
            ("/virtual/imported.dart", "class Imported {}"),
            ("/virtual/alternate.dart", "class Alternate {}"),
            ("/virtual/shared.dart", "part of shared; class FromPart {}"),
        ]
        .into_iter()
        .map(|(path, source)| {
            ProjectFile::new(path.into(), Arc::new(ParsedFile::parse(source)))
                .expect("project fixture parses successfully")
        })
        .collect();
        let libraries = ExternalLibraries::default();
        for (before, after, expected_rebuilds) in [
            ("class Target {}", "class Changed {}", Some(2)),
            (
                "import 'imported.dart'; class Target {}",
                "import 'alternate.dart'; class Target {}",
                Some(2),
            ),
            (
                "export 'imported.dart'; class Target {}",
                "export 'alternate.dart'; class Target {}",
                Some(2),
            ),
            (
                "class Target {}",
                "library shared; part 'shared.dart'; class Target {}",
                None,
            ),
            (
                "library shared; part 'shared.dart'; class Target {}",
                "class Target {}",
                None,
            ),
        ] {
            let mut environment = AnalysisEnvironment::default();
            let mut analyzer = AutofixAnalyzer::new(
                &analysis,
                &config,
                &mut environment,
                Path::new("/virtual/main.dart"),
                FixContext {
                    external_libraries: Some(&libraries),
                    external: None,
                    project_files: &files,
                    cancelled: &|| false,
                },
            );
            let original = analyzer.analyze(before);
            let updated = analyzer.analyze(after);
            assert_eq!(analyzer.overlay_builds, 1, "{after}");
            assert_eq!(analyzer.body_refreshes, 0, "{after}");
            if let Some(expected) = expected_rebuilds {
                assert_eq!(analyzer.declaration_library_rebuilds, expected, "{after}");
            }
            let mut fresh_environment = AnalysisEnvironment::default();
            let mut fresh = AutofixAnalyzer::new(
                &analysis,
                &config,
                &mut fresh_environment,
                Path::new("/virtual/main.dart"),
                FixContext {
                    external_libraries: Some(&libraries),
                    external: None,
                    project_files: &files,
                    cancelled: &|| false,
                },
            );
            let expected = fresh.analyze(after);
            assert!(!updated.is_empty(), "{after}");
            assert_ne!(format!("{original:?}"), format!("{updated:?}"), "{after}");
            assert_eq!(format!("{updated:?}"), format!("{expected:?}"), "{after}");
        }
    }

    struct CompanionFailure;

    impl falcon_analyze::Rule for CompanionFailure {
        fn prepare<'a>(
            &self,
            _program: falcon_syntax::cst::Program<'a>,
            _ctx: &'a AnalyzeContext<'a>,
        ) -> falcon_analyze::RulePreparation<'a> {
            falcon_analyze::RulePreparation::Diagnostics(vec![
                rules_thread_diagnostic(
                    Path::new("companion.dart"),
                    &std::io::Error::other("companion parser failed"),
                ),
                falcon_diagnostics::Diagnostic::new(
                    falcon_diagnostics::SYNTAX_ERROR_DIAGNOSTIC_ID,
                    falcon_diagnostics::Severity::Error,
                    "Companion syntax error.",
                    "companion.dart",
                    falcon_diagnostics::Span { start: 0, end: 0 },
                ),
            ])
        }
    }

    #[test]
    fn companion_failures_do_not_enter_the_target_fix_loop() {
        let config = FalconConfig::default();
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("companion-failure", Box::new(CompanionFailure));
        let analysis = ProjectAnalysis::for_semantic_autofix_tests(registry);
        let mut environment = AnalysisEnvironment::default();
        let outcome = crate::fix_file(
            &analysis,
            &config,
            &mut environment,
            Path::new("target.dart"),
            "void main() {}",
            false,
        );
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.stop.is_none());
    }

    #[test]
    fn companion_parser_worker_failure_prevents_autofix_indexes_until_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.dart");
        let companion = directory.path().join("part.dart");
        let source = "part 'part.dart'; class Main {}";
        std::fs::write(&path, source).unwrap();
        std::fs::write(
            &companion,
            "// falcon-test:external-parser-failure\npart of 'main.dart';",
        )
        .unwrap();
        let config = FalconConfig::default();
        let analysis =
            ProjectAnalysis::for_semantic_autofix_tests(falcon_analyze::RuleRegistry::new());
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            &path,
            FixContext {
                external: None,
                external_libraries: None,
                project_files: &[],
                cancelled: &|| false,
            },
        );
        assert!(analyzer.analyze(source).is_empty());
        let failure = analyzer.source_failure.take().unwrap();
        assert_eq!(failure.path, companion);
        assert!(failure.to_string().contains("external parser regression"));
        assert!(analyzer.overlay.is_none());
        assert!(analyzer.indexes.is_none());
        std::fs::write(&companion, "part of 'main.dart'; class Current {}").unwrap();
        analyzer.analyze(source);
        assert!(analyzer.source_failure.is_none());
        assert!(analyzer.overlay.is_some());
        assert_eq!(analyzer.companions.len(), 1);
        assert!(analyzer.companions[0].source().contains("class Current"));
    }

    #[test]
    fn reparsed_companion_worker_failure_prevents_indexes_and_recovers_current_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.dart");
        let companion = directory.path().join("part.dart");
        let source = "part 'part.dart'; class Main {}";
        std::fs::write(&path, source).unwrap();
        std::fs::write(&companion, "part of 'main.dart'; class Before {}").unwrap();
        let config = FalconConfig::default();
        let analysis =
            ProjectAnalysis::for_semantic_autofix_tests(falcon_analyze::RuleRegistry::new());
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            &path,
            FixContext {
                external: None,
                external_libraries: None,
                project_files: &[],
                cancelled: &|| false,
            },
        );
        analyzer.parser_failure_path = Some(companion.clone());
        assert!(analyzer.analyze(source).is_empty());
        let failure = analyzer.source_failure.take().unwrap();
        assert_eq!(failure.path, companion);
        assert!(
            failure
                .source
                .to_string()
                .contains("autofix companion parser regression")
        );
        assert!(analyzer.companions.is_empty());
        assert!(analyzer.overlay.is_none());
        assert!(analyzer.indexes.is_none());
        analyzer.parser_failure_path = None;
        std::fs::write(&companion, "part of 'main.dart'; class Current {}").unwrap();
        analyzer.analyze(source);
        assert!(analyzer.source_failure.is_none());
        assert!(analyzer.overlay.is_some());
        assert_eq!(analyzer.companions.len(), 1);
        assert!(analyzer.companions[0].source().contains("class Current"));
    }
    #[test]
    fn successive_edits_reuse_external_indexes_and_report_fresh_syntax() {
        let config = FalconConfig::default();
        let analysis =
            ProjectAnalysis::for_semantic_autofix_tests(falcon_rules::build_registry(&config));
        let mut environment = AnalysisEnvironment::default();
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            Path::new("autofix.dart"),
            FixContext {
                external: None,
                external_libraries: None,
                project_files: &[],
                cancelled: &|| false,
            },
        );
        analyzer.analyze("void main() {}");
        let original = Arc::clone(&analyzer.indexes.as_ref().unwrap().identities);
        let diagnostics = analyzer.analyze("void main( {");
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule == falcon_diagnostics::SYNTAX_ERROR_DIAGNOSTIC_ID));
        assert!(Arc::ptr_eq(
            &original,
            &analyzer.indexes.as_ref().unwrap().identities
        ));
        assert!(analyzer.analyze("void main() { final value = 1; }").iter()
            .all(|diagnostic| diagnostic.rule != falcon_diagnostics::SYNTAX_ERROR_DIAGNOSTIC_ID));
        assert!(Arc::ptr_eq(
            &original,
            &analyzer.indexes.as_ref().unwrap().identities
        ));
    }

    #[test]
    fn caller_graph_and_external_parts_are_reused_without_loading() {
        let config = FalconConfig::default();
        let analysis =
            ProjectAnalysis::for_semantic_autofix_tests(falcon_rules::build_registry(&config));
        let mut environment = AnalysisEnvironment::default();
        let libraries = ExternalLibraries::default();
        let indexes = ExternalIndexes::build(ExternalSources {
            libraries: &libraries,
            environment: &environment,
        });
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            Path::new("autofix.dart"),
            FixContext {
                external: Some(indexes.borrowed()),
                external_libraries: Some(&libraries),
                project_files: &[],
                cancelled: &|| false,
            },
        );
        analyzer.analyze("void main() {}");
        analyzer.analyze("void main() { print(1); }");
        assert!(analyzer.libraries.is_none());
        let reused = analyzer.indexes.as_ref().unwrap();
        assert!(Arc::ptr_eq(&indexes.identities, &reused.identities));
        assert!(Arc::ptr_eq(&indexes.types, &reused.types));
        assert!(Arc::ptr_eq(&indexes.signatures, &reused.signatures));
    }

    struct CheckCachedProject;

    impl falcon_analyze::Rule for CheckCachedProject {
        fn prepare<'a>(
            &self,
            _program: falcon_syntax::cst::Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> falcon_analyze::RulePreparation<'a> {
            let identities = ctx.identities().unwrap();
            let library = identities.library(identities.file(ctx.file_path()).unwrap());
            for name in ["Imported", "FromPart"] {
                assert!(
                    identities
                        .resolve_declaration(
                            library,
                            falcon_analyze::QualifiedName { prefix: None, name }
                        )
                        .is_some(),
                    "{name} must resolve from cached project sources"
                );
            }
            assert_eq!(ctx.library().unwrap().siblings().len(), 1);
            assert!(!ctx.library().unwrap().has_unresolved_parts());
            falcon_analyze::RulePreparation::Diagnostics(Vec::new())
        }
    }

    #[test]
    fn borrowed_graph_retains_cached_imports_and_parts_without_disk_sources() {
        let config = FalconConfig::default();
        let mut registry = falcon_analyze::RuleRegistry::new();
        registry.register("check-cached-project", Box::new(CheckCachedProject));
        let analysis = ProjectAnalysis::for_semantic_autofix_tests(registry);
        let files: Vec<_> = [
            ("/virtual/main.dart", "class Stale {}"),
            ("/virtual/other.dart", "class Imported {}"),
            (
                "/virtual/shared.dart",
                "part of 'main.dart'; class FromPart {}",
            ),
        ]
        .into_iter()
        .map(|(path, source)| {
            ProjectFile::new(path.into(), Arc::new(ParsedFile::parse(source)))
                .expect("project fixture parses successfully")
        })
        .collect();
        let mut environment = AnalysisEnvironment::default();
        let libraries = ExternalLibraries::default();
        let mut analyzer = AutofixAnalyzer::new(
            &analysis,
            &config,
            &mut environment,
            Path::new("/virtual/main.dart"),
            FixContext {
                external_libraries: Some(&libraries),
                project_files: &files,
                external: None,
                cancelled: &|| false,
            },
        );
        for source in [
            "import 'other.dart'; part 'shared.dart'; void main() {}",
            "import 'other.dart'; part 'shared.dart'; void main() { Imported(); FromPart(); }",
        ] {
            assert!(analyzer.analyze(source).is_empty());
        }
        assert!(analyzer.libraries.is_none());
        assert_eq!(analyzer.companions.len(), 2);
    }
}
