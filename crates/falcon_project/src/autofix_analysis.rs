//! Per-run source analysis: reparse edits without reloading the dependency graph.

use std::path::Path;
use std::sync::Arc;

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, ExternalIndexParts, ExternalLibraries, ExternalSources,
    ExternalTarget, IdentityIndex, IdentitySource, PackageIdentity, ParsedFile, ProjectFile,
    SignatureIndex, TypeIndex, build_semantic_indexes, canonical_or_lexical, library_unit,
    package_identities, read_source, rules_thread_diagnostic, source_size_diagnostic,
    syntax_error_diagnostics,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

use crate::{FixContext, ProjectAnalysis, analyze_document};

struct ExternalIndexes {
    identities: Arc<IdentityIndex>,
    types: Arc<TypeIndex>,
    signatures: Arc<SignatureIndex>,
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
/// Only the requested file's parsed tree and project overlay are rebuilt.
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
    companion_diagnostics: Vec<Diagnostic>,
    packages: Vec<PackageIdentity>,
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
            std::iter::once(path)
                .chain(context.project_files.iter().map(|file| file.path.as_path())),
        );
        let target = canonical_or_lexical(path);
        let companions = context
            .project_files
            .iter()
            .filter(|file| canonical_or_lexical(&file.path) != target)
            .map(|file| ProjectFile {
                path: file.path.clone(),
                parsed: Arc::clone(&file.parsed),
                has_parse_errors: file.has_parse_errors,
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
            companion_diagnostics: Vec::new(),
            packages: Vec::new(),
        }
    }

    pub(crate) fn analyze(&mut self, source: &str) -> Vec<Diagnostic> {
        falcon_dart_parser::with_stack(|| self.analyze_inner(source))
            .unwrap_or_else(|error| vec![rules_thread_diagnostic(self.path, &error)])
    }

    fn analyze_inner(&mut self, source: &str) -> Vec<Diagnostic> {
        let file = match self.parse(self.path, source) {
            Ok((file, diagnostics)) => (file, diagnostics),
            Err(diagnostic) => return vec![*diagnostic],
        };
        let (file, mut diagnostics) = file;
        if !self.analysis.resolve {
            let ctx = AnalyzeContext::new(self.path, self.config, &file.parsed)
                .with_environment(Some(self.environment));
            return analyze_document(&self.analysis.registry, self.config, &ctx, diagnostics);
        }
        if !self.initialized {
            self.load_external(&file);
        }
        let sources: Vec<_> = std::iter::once(&file)
            .chain(&self.companions)
            .map(|file| IdentitySource {
                path: &file.path,
                parsed: &file.parsed,
                has_parse_errors: file.has_parse_errors,
            })
            .collect();
        let (indexes, grouping) = build_semantic_indexes(
            &sources,
            &self.packages,
            self.external_libraries
                .or(self.libraries.as_ref())
                .map(|libraries| ExternalSources {
                    libraries,
                    environment: self.environment,
                }),
            self.indexes.as_ref().map(ExternalIndexes::borrowed),
        );
        let parsed: Vec<_> = sources.iter().map(|file| file.parsed).collect();
        let library = library_unit(&grouping, &parsed, 0);
        let ctx = AnalyzeContext::new(self.path, self.config, &file.parsed)
            .with_types(&indexes.types)
            .with_identities(&indexes.identities)
            .with_signatures(&indexes.signatures)
            .with_library(&library)
            .with_environment(Some(self.environment));
        diagnostics.extend(self.companion_diagnostics.iter().cloned());
        analyze_document(&self.analysis.registry, self.config, &ctx, diagnostics)
    }

    fn parse(
        &self,
        path: &Path,
        source: &str,
    ) -> Result<(ProjectFile, Vec<Diagnostic>), Box<Diagnostic>> {
        if let Some(diagnostic) = source_size_diagnostic(path, source.len() as u64) {
            return Err(Box::new(diagnostic));
        }
        let parsed = falcon_dart_parser::try_parse_for_package(
            source,
            self.environment.package_language_version(path),
        )
        .map_err(|error| Box::new(rules_thread_diagnostic(path, &error)))?;
        let diagnostics = syntax_error_diagnostics(path, &parsed.errors);
        Ok((
            ProjectFile {
                path: path.to_path_buf(),
                has_parse_errors: !parsed.errors.is_empty(),
                parsed: Arc::new(ParsedFile {
                    source: source.into(),
                    tree: parsed.tree,
                }),
            },
            diagnostics,
        ))
    }

    fn load_external(&mut self, file: &ProjectFile) {
        let path = canonical_or_lexical(self.path);
        let project_paths: std::collections::HashSet<_> = std::iter::once(path.clone())
            .chain(
                self.companions
                    .iter()
                    .map(|file| canonical_or_lexical(&file.path)),
            )
            .collect();
        let roots: Vec<_> = std::iter::once(file)
            .chain(&self.companions)
            .map(|file| (file.path.as_path(), file.program(), file.source()))
            .collect();
        let owned = self.external_libraries.is_none().then(|| {
            ExternalLibraries::load(self.environment, &roots, |candidate| {
                project_paths.contains(candidate)
            })
        });
        let libraries = self.external_libraries.or(owned.as_ref()).unwrap();
        for companion in libraries
            .project_companions()
            .filter(|candidate| owned.is_some() && !project_paths.contains(*candidate))
        {
            if let Ok(source) = read_source(companion) {
                match self.parse(companion, &source) {
                    Ok((file, _)) => self.companions.push(file),
                    Err(diagnostic) => self.companion_diagnostics.push(*diagnostic),
                }
            }
        }
        self.packages = package_identities(
            std::iter::once(self.path)
                .chain(self.companions.iter().map(|file| file.path.as_path())),
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
                        Some(ExternalTarget::Path(target)) if target == path || self.companions.iter().any(|file| file.path == target)
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_edits_reuse_external_indexes_and_report_fresh_syntax() {
        let config = FalconConfig::default();
        let mut analysis = ProjectAnalysis::new(&config);
        analysis.resolve = true;
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
        let mut analysis = ProjectAnalysis::new(&config);
        analysis.resolve = true;
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
        let mut analysis = ProjectAnalysis::new(&config);
        analysis.resolve = true;
        analysis.registry = falcon_analyze::RuleRegistry::new();
        analysis
            .registry
            .register("check-cached-project", Box::new(CheckCachedProject));
        let files: Vec<_> = [
            ("/virtual/main.dart", "class Stale {}"),
            ("/virtual/other.dart", "class Imported {}"),
            (
                "/virtual/shared.dart",
                "part of 'main.dart'; class FromPart {}",
            ),
        ]
        .into_iter()
        .map(|(path, source)| ProjectFile {
            path: path.into(),
            parsed: Arc::new(ParsedFile {
                source: source.into(),
                tree: falcon_dart_parser::parse(source).tree,
            }),
            has_parse_errors: false,
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
