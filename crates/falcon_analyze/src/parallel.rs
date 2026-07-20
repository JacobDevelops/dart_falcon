use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tracing::{info, info_span};

use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_dart_parser::parser::ParseError;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::ast::Program;

use crate::resolve::{
    LibrarySource, LibraryUnit, ProgramSource, ProjectIndex, TypeIndex, group_libraries,
    library_unit,
};
use crate::{AnalyzeContext, ProjectFile, RuleRegistry};

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
) -> (Vec<Diagnostic>, Option<ProjectFile>) {
    let span = info_span!("analyze_file", file = %path.display());
    let _enter = span.enter();
    let (program, parse_errors) = parse(source);
    let ctx = AnalyzeContext::new(path, source, config);
    let mut diagnostics = syntax_error_diagnostics(path, &parse_errors);
    diagnostics.extend(registry.run_all(&program, &ctx));
    info!(file = %path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
    let retained = collect_programs.then(|| ProjectFile {
        path: path.to_path_buf(),
        source: source.to_owned(),
        program,
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

/// A parsed file awaiting analysis. Retained across the parse-then-analyze
/// reorder so a single cross-file [`ProjectIndex`] can be built from every
/// program before the per-file pass runs (resolver-dependent rules need it).
struct Parsed {
    path: PathBuf,
    source: String,
    program: Program,
    parse_errors: Vec<ParseError>,
}

fn parse_one(path: &Path, source: &str) -> Parsed {
    let (program, parse_errors) = parse(source);
    Parsed {
        path: path.to_path_buf(),
        source: source.to_owned(),
        program,
        parse_errors,
    }
}

/// Retain the parsed programs as [`ProjectFile`]s for the cross-file pass, or drop
/// them when the caller does not need them.
fn retain(parsed: Vec<Parsed>, collect_programs: bool) -> Vec<ProjectFile> {
    if !collect_programs {
        return Vec::new();
    }
    parsed
        .into_iter()
        .map(|p| ProjectFile {
            path: p.path,
            source: p.source,
            program: p.program,
            has_parse_errors: !p.parse_errors.is_empty(),
        })
        .collect()
}

/// Build one cross-file [`ProjectIndex`] from every parsed program, then run the
/// per-file rules with that index attached to each context. Shared by the
/// parallel and sequential resolving entry points.
fn analyze_indexed(
    registry: &RuleRegistry,
    parsed: &[Parsed],
    config: &FalconConfig,
    parallel: bool,
) -> Vec<Diagnostic> {
    let index = {
        let sources: Vec<ProgramSource> = parsed
            .iter()
            .map(|p| ProgramSource {
                program: &p.program,
                has_parse_errors: !p.parse_errors.is_empty(),
            })
            .collect();
        ProjectIndex::from_project_files(&sources)
    };

    // Partition files into libraries, then build the type index (each type
    // inheriting its library's unresolved-part flag) and per-file library units.
    let grouping = {
        let files: Vec<(PathBuf, &Program)> = parsed
            .iter()
            .map(|p| (p.path.clone(), &p.program))
            .collect();
        group_libraries(&files)
    };
    let type_index = {
        let sources = parsed.iter().enumerate().map(|(i, p)| LibrarySource {
            program: &p.program,
            has_parse_errors: !p.parse_errors.is_empty(),
            has_unresolved_parts: grouping.is_unresolved(i),
        });
        TypeIndex::from_library_files(sources)
    };
    let programs: Vec<&Program> = parsed.iter().map(|p| &p.program).collect();
    let library_units: Vec<LibraryUnit> = (0..parsed.len())
        .map(|i| library_unit(&grouping, &programs, i))
        .collect();

    let run_one = |(i, p): (usize, &Parsed)| {
        let span = info_span!("analyze_file", file = %p.path.display());
        let _enter = span.enter();
        let ctx = AnalyzeContext::new(&p.path, &p.source, config)
            .with_project(&index)
            .with_types(&type_index)
            .with_library(&library_units[i]);
        let mut diagnostics = syntax_error_diagnostics(&p.path, &p.parse_errors);
        diagnostics.extend(registry.run_all(&p.program, &ctx));
        info!(file = %p.path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
        diagnostics
    };
    if parallel {
        parsed.par_iter().enumerate().flat_map(run_one).collect()
    } else {
        parsed.iter().enumerate().flat_map(run_one).collect()
    }
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
        .flat_map(|(path, source)| analyze_file(registry, path, source, config, false).0)
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
    analyze_parallel_collecting_resolving(registry, files, config, collect_programs, false)
}

/// Sequential counterpart of [`analyze_parallel_collecting`].
pub fn analyze_sequential_collecting(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    analyze_sequential_collecting_resolving(registry, files, config, collect_programs, false)
}

/// Like [`analyze_parallel_collecting`], but with `resolve` controlling whether a
/// shared cross-file [`ProjectIndex`] is built and attached to every file's
/// [`AnalyzeContext::project`]. When `resolve` is set the driver parses all files
/// first, builds one index from every program, then runs the per-file pass with
/// that index; otherwise it takes the per-file fast path (no index, zero cost).
pub fn analyze_parallel_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    if !resolve {
        let (diagnostics, retained): (Vec<Vec<Diagnostic>>, Vec<Option<ProjectFile>>) = files
            .par_iter()
            .map(|(path, source)| analyze_file(registry, path, source, config, collect_programs))
            .unzip();
        return (
            diagnostics.into_iter().flatten().collect(),
            retained.into_iter().flatten().collect(),
        );
    }
    let parsed: Vec<Parsed> = files
        .par_iter()
        .map(|(path, source)| parse_one(path, source))
        .collect();
    let diagnostics = analyze_indexed(registry, &parsed, config, true);
    (diagnostics, retain(parsed, collect_programs))
}

/// Sequential counterpart of [`analyze_parallel_collecting_resolving`].
pub fn analyze_sequential_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    if !resolve {
        let mut diagnostics = Vec::new();
        let mut retained = Vec::new();
        for (path, source) in files {
            let (diags, file) = analyze_file(registry, path, source, config, collect_programs);
            diagnostics.extend(diags);
            retained.extend(file);
        }
        return (diagnostics, retained);
    }
    let parsed: Vec<Parsed> = files.iter().map(|(p, s)| parse_one(p, s)).collect();
    let diagnostics = analyze_indexed(registry, &parsed, config, false);
    (diagnostics, retain(parsed, collect_programs))
}
