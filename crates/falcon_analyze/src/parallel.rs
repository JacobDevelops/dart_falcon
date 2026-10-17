use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use tracing::{info, info_span};

use falcon_config::FalconConfig;
use falcon_dart_parser::parse_for_package;
use falcon_dart_parser::parser::ParseError;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::cst::Program;
use falcon_syntax::language_version::LanguageVersion;

use crate::environment::{AnalysisEnvironment, ExternalLibraries};
use crate::resolve::{
    ExternalSources, IdentityIndex, IdentitySource, LibrarySource, LibraryUnit, PackageIdentity,
    SignatureIndex, TypeIndex, canonical_or_lexical, group_libraries, library_unit,
};
use crate::{AnalyzeContext, ParsedFile, ProjectFile, RuleRegistry, SemanticIndexes};

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
    let span = info_span!("analyze_file", file = %path.display());
    let _enter = span.enter();
    if let Some(diagnostic) = source_size_diagnostic(path, source.len() as u64) {
        return (vec![diagnostic], None);
    }
    let package_version = workspace.package_language_version(path);
    let falcon_dart_parser::ParseResult {
        errors: parse_errors,
        tree,
    } = parse_for_package(source, package_version);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let ctx = AnalyzeContext {
        environment: workspace.environment,
        package_language_version: package_version,
        ..AnalyzeContext::new(path, config, &parsed)
    };
    let mut diagnostics = syntax_error_diagnostics(path, &parse_errors);
    diagnostics.extend(registry.run_all(parsed.program(), &ctx));
    info!(file = %path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
    let retained = collect_programs.then(|| ProjectFile {
        path: path.to_path_buf(),
        parsed,
        has_parse_errors: !parse_errors.is_empty(),
    });
    (diagnostics, retained)
}

/// Convert the parser's recovered errors into `syntax-error` diagnostics
/// (severity error) so non-compiling Dart is reported rather than silently
/// linted as if valid. Public so other analyze consumers (e.g. the LSP server)
/// surface the same diagnostics. Positions are resolved later, once source is at
/// hand at the output boundary.
pub fn syntax_error_diagnostics(path: &Path, errors: &[ParseError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| {
            Diagnostic::new(
                "syntax-error",
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
pub fn source_size_diagnostic(path: &Path, bytes: u64) -> Option<Diagnostic> {
    let error = crate::source::check_source_size(bytes).err()?;
    Some(Diagnostic::new(
        "syntax-error",
        Severity::Error,
        error.to_string(),
        path.to_string_lossy().into_owned(),
        Span { start: 0, end: 0 },
    ))
}

/// A parsed file retained until shared semantic indexes are built.
struct Parsed<'src> {
    path: &'src Path,
    parsed: Arc<ParsedFile>,
    parse_errors: Vec<ParseError>,
}

fn parse_one<'src>(path: &'src Path, source: Box<str>, workspace: Workspace<'_>) -> Parsed<'src> {
    let falcon_dart_parser::ParseResult {
        errors: parse_errors,
        tree,
    } = parse_for_package(&source, workspace.package_language_version(path));
    Parsed {
        path,
        parsed: Arc::new(ParsedFile { source, tree }),
        parse_errors,
    }
}

/// Retain the parsed programs as [`ProjectFile`]s for the cross-file pass, or drop
/// them when the caller does not need them.
fn retain(parsed: Vec<Parsed<'_>>, collect_programs: bool) -> Vec<ProjectFile> {
    if !collect_programs {
        return Vec::new();
    }
    parsed
        .into_iter()
        .map(|p| ProjectFile {
            path: p.path.to_path_buf(),
            parsed: p.parsed,
            has_parse_errors: !p.parse_errors.is_empty(),
        })
        .collect()
}

/// Build shared semantic indexes, then run parallel or sequential per-file rules.
fn analyze_indexed(
    registry: &RuleRegistry,
    parsed: &[Parsed<'_>],
    config: &FalconConfig,
    parallel: bool,
    workspace: Workspace<'_>,
) -> (Vec<Diagnostic>, SemanticIndexes) {
    // Partition files into libraries, then build the type index (each type
    // inheriting its library's unresolved-part flag) and per-file library units.
    let externals = workspace
        .environment
        .map(|environment| {
            let span = info_span!("load_external_libraries");
            let _enter = span.enter();
            let project_paths: HashSet<PathBuf> = parsed
                .iter()
                .map(|p| canonical_or_lexical(p.path))
                .collect();
            let roots: Vec<(&Path, Program<'_>, &str)> = parsed
                .iter()
                .map(|p| (p.path, p.parsed.program(), p.parsed.source.as_ref()))
                .collect();
            let libraries =
                ExternalLibraries::load(environment, &roots, |path| project_paths.contains(path));
            info!(libraries = libraries.libraries().len(), "external libraries loaded");
            for library in libraries.libraries() {
                for file in library.files.iter().filter(|file| file.has_parse_errors) {
                    info!(file = %file.path.display(), "external file has parse errors; its library stays unresolved");
                }
            }
            (libraries, environment, project_paths)
        });
    // Opening a dependency part promotes the defining library into the project
    // view. Index its unopened companions without linting them as requested files.
    let mut companion_sources: Vec<(PathBuf, Box<str>)> = externals
        .as_ref()
        .into_iter()
        .flat_map(|(libraries, _, project_paths)| {
            libraries
                .project_companions()
                .filter(|path| !project_paths.contains(*path))
        })
        .filter_map(|path| {
            crate::source::read_source(path)
                .ok()
                .map(|source| (path.to_path_buf(), source.into_boxed_str()))
        })
        .collect();
    let companions: Vec<Parsed<'_>> = companion_sources
        .iter_mut()
        .map(|(path, source)| parse_one(path, std::mem::take(source), workspace))
        .collect();
    let semantic_parsed: Vec<&Parsed<'_>> = parsed.iter().chain(&companions).collect();
    let project_files: Vec<(PathBuf, Program<'_>)> = semantic_parsed
        .iter()
        .map(|file| (file.path.to_path_buf(), file.parsed.program()))
        .collect();
    let grouping = group_libraries(&project_files);
    let identity_sources: Vec<IdentitySource<'_>> = semantic_parsed
        .iter()
        .map(|p| IdentitySource {
            path: p.path,
            parsed: &p.parsed,
            has_parse_errors: !p.parse_errors.is_empty(),
        })
        .collect();
    let sources: Vec<&str> = semantic_parsed
        .iter()
        .map(|p| p.parsed.source.as_ref())
        .collect();
    let identities = IdentityIndex::build_layer(
        &identity_sources,
        &sources,
        workspace.packages,
        externals
            .as_ref()
            .map(|(libraries, environment, _)| ExternalSources {
                libraries,
                environment,
            }),
        None,
        Some(&grouping),
    );
    tracing::debug!("identity index built");
    let mut type_index = TypeIndex::from_resolved_files(
        semantic_parsed.iter().enumerate().map(|(i, p)| {
            (
                p.path,
                LibrarySource {
                    program: p.parsed.program(),
                    has_parse_errors: !p.parse_errors.is_empty(),
                    has_unresolved_parts: grouping.is_unresolved(i),
                },
            )
        }),
        &identities,
    );
    if let Some((libraries, _, _)) = &externals {
        type_index.add_external(libraries, &identities);
    }
    tracing::debug!("type index built");
    let signature_files: Vec<_> = project_files
        .into_iter()
        .zip(&semantic_parsed)
        .map(|((path, _), p)| (path, Arc::clone(&p.parsed)))
        .collect();
    let signatures = SignatureIndex::build(
        &signature_files,
        externals.as_ref().map(|(libraries, _, _)| libraries),
        &identities,
        &type_index,
    );
    info!("semantic indexes built");
    // The indexes retain the external declaration nodes they need.
    drop(externals);
    let parsed_files: Vec<_> = semantic_parsed.iter().map(|p| &p.parsed).collect();
    let library_units: Vec<LibraryUnit> = (0..parsed.len())
        .map(|i| library_unit(&grouping, &parsed_files, i))
        .collect();

    let run_one = |(i, p): (usize, &Parsed<'_>)| {
        let span = info_span!("analyze_file", file = %p.path.display());
        let _enter = span.enter();
        let ctx = AnalyzeContext::new(p.path, config, &p.parsed)
            .with_types(&type_index)
            .with_identities(&identities)
            .with_signatures(&signatures)
            .with_library(&library_units[i])
            .with_environment(workspace.environment);
        let mut diagnostics = syntax_error_diagnostics(p.path, &p.parse_errors);
        diagnostics.extend(registry.run_all(p.parsed.program(), &ctx));
        info!(file = %p.path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
        diagnostics
    };
    let diagnostics = if parallel {
        parsed.par_iter().enumerate().flat_map(run_one).collect()
    } else {
        parsed.iter().enumerate().flat_map(run_one).collect()
    };
    (
        diagnostics,
        SemanticIndexes {
            types: type_index,
            identities,
            signatures,
        },
    )
}

/// Analyze multiple Dart files in parallel using Rayon work-stealing.
pub fn analyze_parallel(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    analyze_parallel_collecting(registry, files, config, false).0
}

/// Analyze multiple Dart files sequentially (deterministic, useful for debugging).
pub fn analyze_sequential(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    files
        .iter()
        .flat_map(|(path, source)| {
            analyze_file(registry, path, source, config, false, Workspace::default()).0
        })
        .collect()
}

/// Analyze in parallel, additionally retaining each parsed [`ProjectFile`] when
/// `collect_programs` is set so the caller can run the cross-file pass over them.
/// With `collect_programs = false` this behaves exactly like [`analyze_parallel`]
/// (programs are dropped after per-file analysis).
pub fn analyze_parallel_collecting(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    let (diagnostics, files, _) = analyze_parallel_collecting_resolving(
        registry,
        files,
        config,
        collect_programs,
        false,
        Workspace::default(),
    );
    (diagnostics, files)
}

/// Sequential counterpart of [`analyze_parallel_collecting`].
pub fn analyze_sequential_collecting(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    let (diagnostics, files, _) = analyze_sequential_collecting_resolving(
        registry,
        files,
        config,
        collect_programs,
        false,
        Workspace::default(),
    );
    (diagnostics, files)
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

/// Like [`analyze_parallel_collecting`], but optionally builds shared semantic
/// indexes before the per-file pass. Without resolution, uses the per-file fast path.
/// Returns the built indexes for reuse by the cross-file pass when resolution is enabled.
pub fn analyze_parallel_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> (Vec<Diagnostic>, Vec<ProjectFile>, Option<SemanticIndexes>) {
    if !resolve {
        let (diagnostics, retained): (Vec<Vec<Diagnostic>>, Vec<Option<ProjectFile>>) = files
            .par_iter()
            .map(|(path, source)| {
                analyze_file(registry, path, source, config, collect_programs, workspace)
            })
            .unzip();
        return (
            diagnostics.into_iter().flatten().collect(),
            retained.into_iter().flatten().collect(),
            None,
        );
    }
    let mut diagnostics = Vec::new();
    let parsed: Vec<Parsed<'_>> = if files
        .iter()
        .all(|(_, source)| source.len() as u64 <= u64::from(u32::MAX))
    {
        // Preserve the indexed parallel collection and its exact allocation for
        // the normal case; no intermediate list of accepted files is needed.
        files
            .par_iter()
            .map(|(path, source)| parse_one(path, source.as_str().into(), workspace))
            .collect()
    } else {
        files
            .iter()
            .filter_map(|(path, source)| {
                if let Some(diagnostic) = source_size_diagnostic(path, source.len() as u64) {
                    diagnostics.push(diagnostic);
                    None
                } else {
                    Some(parse_one(path, source.as_str().into(), workspace))
                }
            })
            .collect()
    };
    let (file_diagnostics, indexes) = analyze_indexed(registry, &parsed, config, true, workspace);
    if diagnostics.is_empty() {
        diagnostics = file_diagnostics;
    } else {
        diagnostics.extend(file_diagnostics);
    }
    (diagnostics, retain(parsed, collect_programs), Some(indexes))
}

/// Sequential counterpart of [`analyze_parallel_collecting_resolving`].
pub fn analyze_sequential_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
    workspace: Workspace<'_>,
) -> (Vec<Diagnostic>, Vec<ProjectFile>, Option<SemanticIndexes>) {
    if !resolve {
        let mut diagnostics = Vec::new();
        let mut retained = Vec::new();
        for (path, source) in files {
            let (diags, file) =
                analyze_file(registry, path, source, config, collect_programs, workspace);
            diagnostics.extend(diags);
            retained.extend(file);
        }
        return (diagnostics, retained, None);
    }
    let mut diagnostics = Vec::new();
    let parsed: Vec<Parsed<'_>> = files
        .iter()
        .filter_map(|(path, source)| {
            if let Some(diagnostic) = source_size_diagnostic(path, source.len() as u64) {
                diagnostics.push(diagnostic);
                None
            } else {
                Some(parse_one(path, source.as_str().into(), workspace))
            }
        })
        .collect();
    let (file_diagnostics, indexes) = analyze_indexed(registry, &parsed, config, false, workspace);
    if diagnostics.is_empty() {
        diagnostics = file_diagnostics;
    } else {
        diagnostics.extend(file_diagnostics);
    }
    (diagnostics, retain(parsed, collect_programs), Some(indexes))
}
