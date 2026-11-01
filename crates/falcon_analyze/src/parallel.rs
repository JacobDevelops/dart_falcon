use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use tracing::{debug, debug_span, info};

use falcon_config::FalconConfig;
use falcon_dart_parser::parser::ParseError;
use falcon_diagnostics::{Diagnostic, SYNTAX_ERROR_DIAGNOSTIC_ID, Severity, Span};
use falcon_syntax::cst::Program;
use falcon_syntax::language_version::LanguageVersion;

use crate::environment::{AnalysisEnvironment, ExternalLibraries};
use crate::resolve::{
    ExternalSources, IdentitySource, LibraryUnit, PackageIdentity, canonical_or_lexical,
    library_unit,
};
use crate::{
    AnalyzeContext, ParsedFile, ProjectFile, RuleRegistry, SemanticIndexes, build_semantic_indexes,
};

type AnalysisOutput = (Vec<Diagnostic>, Vec<ProjectFile>, Option<SemanticIndexes>);

/// Parse `source`, run every per-file rule (with no project index), and
/// optionally keep the parsed program for the cross-file pass. This is the fast
/// path: resolver-dependent rules are not enabled, so no cross-file index is
/// built and each file is parsed and analyzed in one step.
fn analyze_file(
    registry: &RuleRegistry,
    path: &Path,
    source: &str,
    config: &FalconConfig,
    collect_programs: bool,
    workspace: Workspace<'_>,
) -> (Vec<Diagnostic>, Option<ProjectFile>) {
    let span = debug_span!(target: "falcon::detail", "analyze_file", file = %path.display());
    let _enter = span.enter();
    let package_version = workspace.package_language_version(path);
    let SourceParse {
        parsed,
        parse_errors,
        ..
    } = match parse_one(path, source, package_version) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return (vec![*diagnostic], None),
    };
    let ctx = AnalyzeContext::new(path, config, &parsed)
        .with_package_environment(workspace.environment, package_version);
    let mut diagnostics = syntax_error_diagnostics(path, &parse_errors);
    diagnostics.extend(registry.run_all(parsed.program(), &ctx));
    debug!(target: "falcon::detail", file = %path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
    let retained = collect_programs.then(|| {
        ProjectFile::new(path.to_path_buf(), parsed)
            .expect("successful parser result must be accepted by the project")
    });
    (diagnostics, retained)
}

/// Convert the parser's recovered errors into `syntax-error` diagnostics
/// (severity error) so non-compiling Dart is reported rather than silently
/// linted as if valid. Public so other analyze consumers (e.g. the LSP server)
/// surface the same diagnostics. Positions are resolved later, once source is at
/// hand at the output boundary.
#[must_use]
pub fn syntax_error_diagnostics(path: &Path, errors: &[ParseError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| {
            Diagnostic::new(
                SYNTAX_ERROR_DIAGNOSTIC_ID,
                Severity::Error,
                e.message.clone(),
                path.to_string_lossy().into_owned(),
                Span {
                    start: e.offset,
                    end: e.offset,
                },
            )
        })
        .collect()
}

/// Report unsupported source sizes through the same error channel as syntax errors.
#[must_use]
pub fn source_size_diagnostic(path: &Path, bytes: u64) -> Option<Diagnostic> {
    let error = crate::source::check_source_size(bytes).err()?;
    Some(Diagnostic::new(
        SYNTAX_ERROR_DIAGNOSTIC_ID,
        Severity::Error,
        error.to_string(),
        path.to_string_lossy().into_owned(),
        Span { start: 0, end: 0 },
    ))
}

/// A parsed file retained until shared semantic indexes are built.
struct SourceParse<'src> {
    path: &'src Path,
    parsed: Arc<ParsedFile>,
    parse_errors: Vec<ParseError>,
}

/// Reject oversized inputs once, before source ownership or parser work, for
/// both execution modes and both analysis paths.
fn parse_one(
    path: &Path,
    source: impl AsRef<str> + Into<Box<str>>,
    package_version: Option<LanguageVersion>,
) -> Result<SourceParse<'_>, Box<Diagnostic>> {
    let _parse = debug_span!(target: "falcon::stages", "parse").entered();
    if let Some(diagnostic) = source_size_diagnostic(path, source.as_ref().len() as u64) {
        return Err(Box::new(diagnostic));
    }
    let (parsed, parse_errors) = ParsedFile::try_parse_for_package(source, package_version)
        .map_err(|(_, error)| Box::new(crate::rules_thread_diagnostic(path, &error)))?;
    Ok(SourceParse {
        path,
        parsed: Arc::new(parsed),
        parse_errors,
    })
}

/// Retain the parsed programs as [`ProjectFile`]s for the cross-file pass, or drop
/// them when the caller does not need them.
fn retain(parsed: Vec<SourceParse<'_>>, collect_programs: bool) -> Vec<ProjectFile> {
    if !collect_programs {
        return Vec::new();
    }
    parsed
        .into_iter()
        .map(|p| {
            ProjectFile::new(p.path.to_path_buf(), p.parsed)
                .expect("successful parser result must be accepted by the project")
        })
        .collect()
}

/// Build shared semantic indexes, then run parallel or sequential per-file rules.
fn analyze_indexed(
    registry: &RuleRegistry,
    parsed: &[SourceParse<'_>],
    config: &FalconConfig,
    parallel: bool,
    workspace: Workspace<'_>,
) -> Result<(Vec<Diagnostic>, SemanticIndexes), crate::SourceFailure> {
    let acquisition = debug_span!(target: "falcon::stages", "environment_capture").entered();
    let externals = load_indexed_externals(parsed, workspace);
    let mut companion_sources = read_companion_sources(parsed, workspace, externals.as_ref())?;
    drop(acquisition);
    let mut companion_diagnostics = Vec::new();
    let companions: Vec<SourceParse<'_>> = companion_sources
        .iter_mut()
        .filter_map(|(path, source)| {
            match parse_one(
                path,
                std::mem::take(source),
                workspace.package_language_version(path),
            ) {
                Ok(parsed) => Some(parsed),
                Err(diagnostic) => {
                    companion_diagnostics.push(*diagnostic);
                    None
                }
            }
        })
        .collect();
    let semantic_parsed: Vec<&SourceParse<'_>> = parsed.iter().chain(&companions).collect();
    let identity_sources: Vec<IdentitySource<'_>> = semantic_parsed
        .iter()
        .map(|p| IdentitySource {
            path: p.path,
            parsed: &p.parsed,
            has_parse_errors: p.parsed.has_parse_errors(),
        })
        .collect();
    let (
        SemanticIndexes {
            types: type_index,
            identities,
            signatures,
        },
        grouping,
    ) = build_semantic_indexes(
        &identity_sources,
        workspace.packages,
        externals
            .as_ref()
            .map(|(libraries, environment, _)| ExternalSources {
                libraries,
                environment,
            }),
        None,
    );
    // The indexes retain the external declaration nodes they need.
    drop(externals);
    let parsed_files: Vec<_> = semantic_parsed.iter().map(|p| &p.parsed).collect();
    let library_units: Vec<LibraryUnit> = (0..parsed.len())
        .map(|i| library_unit(&grouping, &parsed_files, i))
        .collect();

    let context = crate::registry::TraceContext::capture();
    let run_one = |(i, p): (usize, &SourceParse<'_>)| {
        context.in_scope(|| {
            let span =
                debug_span!(target: "falcon::detail", "analyze_file", file = %p.path.display());
            let _enter = span.enter();
            let ctx = AnalyzeContext::new(p.path, config, &p.parsed)
                .with_types(&type_index)
                .with_identities(&identities)
                .with_signatures(&signatures)
                .with_library(&library_units[i])
                .with_environment(workspace.environment);
            let mut diagnostics = syntax_error_diagnostics(p.path, &p.parse_errors);
            diagnostics.extend(registry.run_all(p.parsed.program(), &ctx));
            debug!(
                target: "falcon::detail",
                file = %p.path.display(),
                diagnostic_count = diagnostics.len(),
                "file analysis complete"
            );
            diagnostics
        })
    };
    let mut diagnostics: Vec<Diagnostic> = if parallel {
        parsed.par_iter().enumerate().flat_map(run_one).collect()
    } else {
        parsed.iter().enumerate().flat_map(run_one).collect()
    };
    diagnostics.extend(companion_diagnostics);
    Ok((
        diagnostics,
        SemanticIndexes {
            types: type_index,
            identities,
            signatures,
        },
    ))
}

type IndexedExternals<'a> = (ExternalLibraries, &'a AnalysisEnvironment, HashSet<PathBuf>);

fn read_companion_sources(
    parsed: &[SourceParse<'_>],
    workspace: Workspace<'_>,
    externals: Option<&IndexedExternals<'_>>,
) -> Result<Vec<(PathBuf, Box<str>)>, crate::SourceFailure> {
    let roots = crate::project_source_roots(
        parsed.iter().map(|file| file.path).chain(
            workspace
                .packages
                .iter()
                .map(|package| package.lib_root.as_path()),
        ),
    );
    let required = |path: &Path| {
        workspace.environment.map_or_else(
            || {
                roots
                    .iter()
                    .any(|root| crate::canonical_or_lexical(path).starts_with(root))
            },
            |environment| environment.project_source_required(path, &roots),
        )
    };
    if let Some((libraries, _, _)) = externals
        && let Some(error) = libraries.worker_failure()
    {
        return Err(error);
    }
    if let Some((libraries, _, _)) = externals
        && let Some(error) = libraries.source_failure(required)
    {
        return Err(error);
    }
    // Opening a dependency part promotes its unopened companions without linting them.
    externals
        .into_iter()
        .flat_map(|(libraries, _, project_paths)| {
            libraries
                .project_companions()
                .filter(|path| !project_paths.contains(*path))
        })
        .filter_map(|path| match crate::acquire_source(path).companion(path) {
            Ok(Some(source)) => Some(Ok((path.to_path_buf(), source.into_boxed_str()))),
            Ok(None) => None,
            Err(error) if required(path) => Some(Err(error)),
            Err(error) => {
                tracing::warn!("{error}");
                None
            }
        })
        .collect()
}

fn load_indexed_externals<'a>(
    parsed: &[SourceParse<'_>],
    workspace: Workspace<'a>,
) -> Option<IndexedExternals<'a>> {
    workspace
        .environment
        .map(|environment| {
            let span = debug_span!(target: "falcon::stages", "load_external_libraries");
            let _enter = span.enter();
            let project_paths: HashSet<PathBuf> = parsed
                .iter()
                .map(|p| canonical_or_lexical(p.path))
                .collect();
            let roots: Vec<(&Path, Program<'_>, &str)> = parsed
                .iter()
                .map(|p| (p.path, p.parsed.program(), p.parsed.source()))
                .collect();
            let libraries =
                ExternalLibraries::load(environment, &roots, |path| project_paths.contains(path));
            info!(
                libraries = libraries.libraries().len(),
                parse_error_files = libraries
                    .libraries()
                    .iter()
                    .flat_map(|library| library.files.iter())
                    .filter(|file| file.has_parse_errors)
                    .count(),
                "external libraries loaded"
            );
            for library in libraries.libraries() {
                for file in library.files.iter().filter(|file| file.has_parse_errors) {
                    info!(file = %file.path.display(), "external file has parse errors; its library stays unresolved");
                }
            }
            (libraries, environment, project_paths)
        })
}

/// What a resolving run knows about the analyzed project beyond its files.
#[derive(Clone, Copy, Default)]
pub struct Workspace<'a> {
    /// Project packages (from their `pubspec.yaml`), for `package:` imports
    /// between project files.
    pub packages: &'a [PackageIdentity],
    /// Package configurations, language versions and the SDK. With an SDK,
    /// resolving runs load and index the external libraries project files
    /// import.
    pub environment: Option<&'a AnalysisEnvironment>,
}

impl Workspace<'_> {
    fn package_language_version(&self, path: &Path) -> Option<LanguageVersion> {
        self.environment?.package_language_version(path)
    }
}

/// Analyze files in parallel, optionally retaining parsed programs for cross-file
/// analysis and building shared semantic indexes before per-file rules run.
/// Without resolution, each file is parsed and analyzed in one step.
///
/// # Errors
/// Returns the path and I/O failure of any unreadable required project companion.
pub fn analyze_parallel_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> Result<AnalysisOutput, crate::SourceFailure> {
    crate::with_rules_stack(|| {
        analyze_parallel_inner(
            registry,
            files,
            config,
            collect_programs,
            resolve,
            workspace,
        )
    })
    .unwrap_or_else(|error| {
        Ok((
            files
                .iter()
                .map(|(path, _)| crate::rules_thread_diagnostic(path, &error))
                .collect(),
            Vec::new(),
            None,
        ))
    })
}

fn analyze_parallel_inner(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> Result<AnalysisOutput, crate::SourceFailure> {
    if !resolve {
        let context = crate::registry::TraceContext::capture();
        let (diagnostics, retained): (Vec<Vec<Diagnostic>>, Vec<Option<ProjectFile>>) = files
            .par_iter()
            .map(|(path, source)| {
                context.in_scope(|| {
                    analyze_file(registry, path, source, config, collect_programs, workspace)
                })
            })
            .unzip();
        return Ok((
            diagnostics.into_iter().flatten().collect(),
            retained.into_iter().flatten().collect(),
            None,
        ));
    }
    let mut diagnostics = Vec::new();
    let context = crate::registry::TraceContext::capture();
    let parsed: Vec<Result<SourceParse<'_>, Box<Diagnostic>>> = files
        .par_iter()
        .map(|(path, source)| {
            context.in_scope(|| {
                parse_one(
                    path,
                    source.as_str(),
                    workspace.package_language_version(path),
                )
            })
        })
        .collect();
    // Preserve size errors before worker failures without a supported-input allocation.
    let mut parse_diagnostics = Vec::new();
    let parsed: Vec<_> = parsed
        .into_iter()
        .filter_map(|result| match result {
            Ok(parsed) => Some(parsed),
            Err(diagnostic) => {
                if diagnostic.rule == SYNTAX_ERROR_DIAGNOSTIC_ID {
                    diagnostics.push(*diagnostic);
                } else {
                    parse_diagnostics.push(*diagnostic);
                }
                None
            }
        })
        .collect();
    diagnostics.extend(parse_diagnostics);
    let (file_diagnostics, indexes) = analyze_indexed(registry, &parsed, config, true, workspace)?;
    if diagnostics.is_empty() {
        diagnostics = file_diagnostics;
    } else {
        diagnostics.extend(file_diagnostics);
    }
    Ok((diagnostics, retain(parsed, collect_programs), Some(indexes)))
}

/// Sequential counterpart of [`analyze_parallel_collecting_resolving`].
///
/// # Errors
/// Returns the path and I/O failure of any unreadable required project companion.
pub fn analyze_sequential_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> Result<AnalysisOutput, crate::SourceFailure> {
    crate::with_rules_stack(|| {
        analyze_sequential_inner(
            registry,
            files,
            config,
            collect_programs,
            resolve,
            workspace,
        )
    })
    .unwrap_or_else(|error| {
        Ok((
            files
                .iter()
                .map(|(path, _)| crate::rules_thread_diagnostic(path, &error))
                .collect(),
            Vec::new(),
            None,
        ))
    })
}

fn analyze_sequential_inner(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> Result<AnalysisOutput, crate::SourceFailure> {
    if !resolve {
        let mut diagnostics = Vec::new();
        let mut retained = Vec::new();
        for (path, source) in files {
            let (diags, file) =
                analyze_file(registry, path, source, config, collect_programs, workspace);
            diagnostics.extend(diags);
            retained.extend(file);
        }
        return Ok((diagnostics, retained, None));
    }
    let mut diagnostics = Vec::new();
    let parsed: Vec<SourceParse<'_>> = files
        .iter()
        .filter_map(|(path, source)| {
            match parse_one(
                path,
                source.as_str(),
                workspace.package_language_version(path),
            ) {
                Ok(parsed) => Some(parsed),
                Err(diagnostic) => {
                    diagnostics.push(*diagnostic);
                    None
                }
            }
        })
        .collect();
    let (file_diagnostics, indexes) = analyze_indexed(registry, &parsed, config, false, workspace)?;
    if diagnostics.is_empty() {
        diagnostics = file_diagnostics;
    } else {
        diagnostics.extend(file_diagnostics);
    }
    Ok((diagnostics, retain(parsed, collect_programs), Some(indexes)))
}
