//! Cross-file rules — falcon's port of dart_code_linter's `check-unused-files` /
//! `check-unused-code` commands plus a conservative `check-unnecessary-nullable`.
//! These run in the CLI and LSP cross-file passes (see
//! `falcon_analyze::CrossFileRule`).
//!
//! This module root holds the shared cross-file plumbing every rule needs:
//! package/pubspec discovery, directive-URI resolution, and path normalization.

#[cfg(test)]
mod dependency_tests;

use crate::meta::RuleSource;

crate::declare_rules! {
    unused_files::UnusedFiles => ("unused-files", cross_file, Correctness, false, RuleSource::DartCodeLinter("check-unused-files"));
    unused_code::UnusedCode => ("unused-code", cross_file, Correctness, false, RuleSource::DartCodeLinter("check-unused-code"));
    unnecessary_nullable::UnnecessaryNullable => ("unnecessary-nullable", resolved_cross_file, Correctness, false, RuleSource::DartCodeLinter("check-unnecessary-nullable"));
    depend_on_referenced_packages::DependOnReferencedPackages => ("depend-on-referenced-packages", cross_file, Correctness, true, RuleSource::Lints("depend_on_referenced_packages"));
    package_names::PackageNames => ("package-names", cross_file, Style, true, RuleSource::Lints("package_names"));
    secure_pubspec_urls::SecurePubspecUrls => ("secure-pubspec-urls", cross_file, Security, true, RuleSource::Lints("secure_pubspec_urls"));
    unused_dependencies::UnusedDependencies => ("unused-dependencies", cross_file, Correctness, false, RuleSource::Falcon);
    undeclared_dependencies::UndeclaredDependencies => ("undeclared-dependencies", cross_file, Correctness, false, RuleSource::Falcon);
    no_import_cycles::NoImportCycles => ("no-import-cycles", cross_file, Correctness, false, RuleSource::Falcon);
    banned_imports::BannedImports => ("banned-imports", cross_file, Correctness, false, RuleSource::Falcon);
    sort_pub_dependencies::SortPubDependencies => ("sort-pub-dependencies", cross_file, Correctness, false, RuleSource::Lints("sort_pub_dependencies"));
}

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use falcon_analyze::{PackageIdentity, ProjectFile, canonical_or_lexical};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::Program;
use marked_yaml::{Node as MarkedNode, parse_yaml};
use serde_yaml::Value;

/// Unknown packages use a tolerant `lib/...` suffix instead of a concrete path.
pub enum ResolvedRef {
    Path(PathBuf),
    Suffix(String),
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Find the closest `pubspec.yaml` enclosing `path`.
pub fn enclosing_pubspec(path: &Path) -> Option<PathBuf> {
    let canon = canonical_or_lexical(path);
    if canon.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml") && canon.is_file() {
        return Some(canon);
    }
    let mut dir = canon.parent();
    while let Some(d) = dir {
        let pubspec = d.join("pubspec.yaml");
        if pubspec.is_file() {
            return Some(pubspec);
        }
        dir = d.parent();
    }
    None
}

/// Normalize package roots for virtual files without resolving their inner symlinks.
pub fn canonical_package_path(path: &Path) -> PathBuf {
    if let Ok(path) = std::fs::canonicalize(path) {
        return path;
    }
    let path = canonical_or_lexical(path);
    let Some(manifest) = enclosing_pubspec(&path) else {
        return path;
    };
    let root = manifest.parent().unwrap();
    let Ok(relative) = path.strip_prefix(root) else {
        return path;
    };
    if let Ok(relative) = relative.strip_prefix("lib") {
        canonical_lib_root(root).join(relative)
    } else {
        canonical_or_lexical(root).join(relative)
    }
}

fn canonical_lib_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root.join("lib"))
        .unwrap_or_else(|_| canonical_or_lexical(root).join("lib"))
}

fn package_identity(manifest: &Path) -> PackageIdentity {
    let root = canonical_or_lexical(manifest.parent().unwrap());
    crate::package_identity(&root.join("pubspec.yaml"))
}

/// Locate a parsed YAML string in its original source using the same mapping keys
/// and sequence indexes in a parser that preserves scalar source markers.
pub fn yaml_string_span(source: &str, root: &Value, target: &Value) -> Option<DiagSpan> {
    let mut path = Vec::new();
    yaml_value_path(root, target, &mut path)?;
    let marked = parse_yaml(0, source).ok()?;
    let node = marked_node_at_path(&marked, &path)?;
    if node.as_scalar()?.as_str() != target.as_str()? {
        return None;
    }
    let character = node.span().start()?.character();
    let start = character_to_byte(source, character)?;
    scalar_source_span(source, start)
}

#[derive(Debug)]
enum YamlPathSegment {
    Key(String),
    Index(usize),
}

fn yaml_value_path(root: &Value, target: &Value, path: &mut Vec<YamlPathSegment>) -> Option<()> {
    if std::ptr::eq(root, target) {
        return Some(());
    }
    match root {
        Value::Mapping(entries) => {
            for (key, value) in entries {
                // Non-string keys aren't addressable by `marked_node_at_path`, so
                // skip them instead of abandoning the whole search.
                let Some(key) = key.as_str() else { continue };
                path.push(YamlPathSegment::Key(key.to_owned()));
                if yaml_value_path(value, target, path).is_some() {
                    return Some(());
                }
                path.pop();
            }
        }
        Value::Sequence(values) => {
            for (index, value) in values.iter().enumerate() {
                path.push(YamlPathSegment::Index(index));
                if yaml_value_path(value, target, path).is_some() {
                    return Some(());
                }
                path.pop();
            }
        }
        Value::Tagged(tagged) => return yaml_value_path(&tagged.value, target, path),
        _ => {}
    }
    None
}

fn marked_node_at_path<'a>(
    root: &'a MarkedNode,
    path: &[YamlPathSegment],
) -> Option<&'a MarkedNode> {
    path.iter().try_fold(root, |node, segment| match segment {
        YamlPathSegment::Key(key) => node.as_mapping()?.get_node(key),
        YamlPathSegment::Index(index) => node.as_sequence()?.get(*index),
    })
}

fn character_to_byte(source: &str, character: usize) -> Option<usize> {
    source
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(source.len()))
        .nth(character)
}

fn scalar_source_span(source: &str, start: usize) -> Option<DiagSpan> {
    if let Some(span) = enclosing_block_scalar(source, start) {
        return Some(span);
    }
    let bytes = source.as_bytes();
    let end = match bytes.get(start)? {
        b'\'' => quoted_scalar_end(bytes, start, b'\'', true)?,
        b'"' => quoted_scalar_end(bytes, start, b'"', false)?,
        _ => plain_scalar_end(bytes, start),
    };
    (end > start).then_some(DiagSpan { start, end })
}

fn quoted_scalar_end(bytes: &[u8], start: usize, quote: u8, doubled_escape: bool) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        if !doubled_escape && bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == quote {
            if doubled_escape && bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn plain_scalar_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() {
        let byte = bytes[end];
        if matches!(byte, b'\r' | b'\n' | b',' | b']' | b'}')
            || (byte == b'#' && end > start && bytes[end - 1].is_ascii_whitespace())
            || (byte == b':'
                && bytes.get(end + 1).is_none_or(|next| {
                    next.is_ascii_whitespace() || matches!(next, b',' | b']' | b'}')
                }))
        {
            break;
        }
        end += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

fn enclosing_block_scalar(source: &str, value_start: usize) -> Option<DiagSpan> {
    let value_line_start = source[..value_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let value_indent = source[value_line_start..value_start]
        .bytes()
        .take_while(u8::is_ascii_whitespace)
        .count();
    let mut search_end = value_line_start.saturating_sub(1);

    loop {
        let line_start = source[..search_end]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line = &source[line_start..search_end];
        if !line.trim().is_empty() {
            let indent = line.bytes().take_while(u8::is_ascii_whitespace).count();
            let marker = line.find(':').and_then(|colon| {
                let rest = line[colon + 1..].trim_start();
                matches!(rest.as_bytes().first(), Some(b'|' | b'>'))
                    .then(|| colon + 1 + line[colon + 1..].len() - rest.len())
            });
            if indent < value_indent {
                let marker = marker?;
                let mut scalar_end = search_end;
                let mut next = search_end + usize::from(search_end < source.len());
                while next < source.len() {
                    let next_end = source[next..]
                        .find('\n')
                        .map_or(source.len(), |length| next + length);
                    let next_line = &source[next..next_end];
                    let next_indent = next_line
                        .bytes()
                        .take_while(u8::is_ascii_whitespace)
                        .count();
                    if !next_line.trim().is_empty() && next_indent <= indent {
                        break;
                    }
                    scalar_end = next_end;
                    next = next_end + usize::from(next_end < source.len());
                }
                return Some(DiagSpan {
                    start: line_start + marker,
                    end: scalar_end,
                });
            }
        }
        if line_start == 0 {
            return None;
        }
        search_end = line_start - 1;
    }
}

/// Missing package names leave URI resolution in tolerant suffix-matching mode.
pub fn detect_package(files: &[ProjectFile]) -> Option<PackageIdentity> {
    let pubspec = files
        .iter()
        .find_map(|file| enclosing_pubspec(&file.path))?;
    let package = package_identity(&pubspec);
    (!package.name.is_empty()).then_some(package)
}

fn project_pubspecs(files: &[ProjectFile]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    files
        .iter()
        .filter_map(|file| enclosing_pubspec(&file.path))
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

/// Resolve an `import`/`export`/`part` URI (already the string value, without
/// quotes) relative to `from_file`. `dart:` URIs resolve to `None` (external).
pub fn resolve_directive_uri(
    from_file: &Path,
    uri: &str,
    pkg: Option<&PackageIdentity>,
) -> Option<ResolvedRef> {
    if uri.starts_with("dart:") {
        return None;
    }
    if let Some(rest) = uri.strip_prefix("package:") {
        let mut parts = rest.splitn(2, '/');
        let pkg_name = parts.next().unwrap_or("");
        let sub = parts.next().unwrap_or("");
        if sub.is_empty() {
            return None;
        }
        match pkg {
            Some(pkg) if pkg_name == pkg.name => {
                return Some(ResolvedRef::Path(canonical_or_lexical(
                    &pkg.lib_root.join(sub),
                )));
            }
            // Known local package: a `package:` URI naming a *different* package
            // cannot denote a file in this package, so it is not a local edge.
            Some(_) => return None,
            // No pubspec: match tolerantly on the `lib/<sub>` suffix.
            None => return Some(ResolvedRef::Suffix(format!("lib/{sub}"))),
        }
    }
    let from_dir = from_file.parent().unwrap_or_else(|| Path::new(""));
    Some(ResolvedRef::Path(canonical_package_path(
        &from_dir.join(uri),
    )))
}

/// Conditional alternatives also keep platform-specific files reachable.
fn program_reference_uris(program: Program<'_>) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    let imports = program.imports().flat_map(|i| {
        std::iter::once(i.uri().value()).chain(i.configurable_uris().map(|c| c.uri().value()))
    });
    let exports = program.exports().flat_map(|e| {
        std::iter::once(e.uri().value()).chain(e.configurable_uris().map(|c| c.uri().value()))
    });
    let parts = program.part_directives().map(|p| p.uri().value());
    imports.chain(exports).chain(parts)
}

#[derive(Default)]
pub struct ReferenceSet {
    paths: HashSet<PathBuf>,
    suffixes: Vec<String>,
}

impl ReferenceSet {
    pub fn insert(&mut self, r: ResolvedRef) {
        match r {
            ResolvedRef::Path(p) => {
                self.paths.insert(p);
            }
            ResolvedRef::Suffix(s) => self.suffixes.push(s),
        }
    }

    pub fn contains(&self, path: &Path) -> bool {
        if self.paths.contains(path) {
            return true;
        }
        let s = slashed(path);
        self.suffixes
            .iter()
            .any(|suffix| s == *suffix || s.ends_with(&format!("/{suffix}")))
    }
}

/// Whether `path` lives under the package `lib/` directory (the scope both
/// unused-* rules flag within). With a known package this is a prefix check;
/// without pubspec metadata every analyzed file is treated as in-scope (callers
/// that need the stricter documented `lib/`-only scope apply it themselves).
pub fn is_under_lib(path: &Path, pkg: Option<&PackageIdentity>) -> bool {
    match pkg {
        Some(pkg) => path.starts_with(&pkg.lib_root),
        None => true,
    }
}

/// Whether `path` has a `lib/` path component. Used as the pubspec-less scope
/// fallback for `unused-files`, whose documented scope is files under `lib/`.
pub fn has_lib_component(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "lib")
}

struct CachedPackage {
    modified: SystemTime,
    length: u64,
    #[cfg(unix)]
    changed: (i64, i64),
    name: String,
}

static PACKAGE_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedPackage>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The package enclosing `path` (nearest `pubspec.yaml`), or `None` outside any
/// package. Mirrors the analyzer's `RuleContext.package`.
/// Parsed identities are shared between rules; manifest timestamp or length
/// changes invalidate the bounded process-wide cache.
pub fn enclosing_package(path: &Path) -> Option<PackageIdentity> {
    let manifest = enclosing_pubspec(path)?;
    let pubspec = canonical_or_lexical(&manifest);
    let Ok(metadata) = std::fs::metadata(&pubspec) else {
        return Some(package_identity(&manifest));
    };
    let Ok(modified) = metadata.modified() else {
        return Some(package_identity(&manifest));
    };
    let length = metadata.len();
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        (metadata.ctime(), metadata.ctime_nsec())
    };
    let cached_name = {
        let cache = PACKAGE_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.get(&pubspec).and_then(|entry| {
            let valid = entry.modified == modified && entry.length == length;
            #[cfg(unix)]
            let valid = valid && entry.changed == changed;
            valid.then(|| entry.name.clone())
        })
    };
    if let Some(name) = cached_name {
        return Some(PackageIdentity {
            name,
            // Resolve lib/ afresh: its symlink can change independently of pubspec.
            lib_root: canonical_lib_root(manifest.parent()?),
        });
    }
    let package = package_identity(&manifest);
    let mut cache = PACKAGE_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // Bound retained paths across long-running LSP sessions and test projects.
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(
        pubspec,
        CachedPackage {
            modified,
            length,
            #[cfg(unix)]
            changed,
            name: package.name.clone(),
        },
    );
    Some(package)
}

/// The enclosing package when `path` lies in that package's `lib/` directory.
/// Mirrors the analyzer's `RuleContext.isInLibDir`: `None` for files outside
/// any package or outside `lib/`.
pub fn lib_dir_package(path: &Path) -> Option<PackageIdentity> {
    let package = enclosing_package(path)?;
    canonical_package_path(path)
        .starts_with(&package.lib_root)
        .then_some(package)
}

/// Directed reference graph over the file set: `adj[i]` lists the indices of the
/// files that `files[i]` references via its imports/exports/parts (all URI kinds,
/// including conditional). `canons` is the caller's canonical path per file (same
/// order as `files`), reused to avoid re-canonicalizing. Edge targets are the
/// *first* file index for a given canonical path, so two spellings of one file
/// collapse to a single node. A tolerant suffix reference (pubspec-less only)
/// resolves to every file it matches. Unresolvable/external URIs contribute no
/// edge. Duplicate edges are harmless for the reachability traversal that uses it.
pub fn build_reference_graph(
    files: &[ProjectFile],
    canons: &[PathBuf],
    pkg: Option<&PackageIdentity>,
) -> Vec<Vec<usize>> {
    let mut by_path: HashMap<&Path, usize> = HashMap::with_capacity(canons.len());
    for (i, c) in canons.iter().enumerate() {
        by_path.entry(c.as_path()).or_insert(i);
    }
    let slashed_paths: Vec<String> = canons.iter().map(|c| slashed(c)).collect();

    let mut adj = vec![Vec::new(); files.len()];
    for (i, f) in files.iter().enumerate() {
        for uri in program_reference_uris(f.program()) {
            match resolve_directive_uri(&canons[i], &uri, pkg) {
                Some(ResolvedRef::Path(p)) => {
                    if let Some(&j) = by_path.get(p.as_path()) {
                        adj[i].push(j);
                    }
                }
                Some(ResolvedRef::Suffix(s)) => {
                    let needle = format!("/{s}");
                    for (j, sp) in slashed_paths.iter().enumerate() {
                        if *sp == s || sp.ends_with(&needle) {
                            adj[i].push(j);
                        }
                    }
                }
                None => {}
            }
        }
    }
    adj
}

#[cfg(test)]
use crate::inference;
#[cfg(test)]
use falcon_analyze::RuleAnalysis;

/// Test support for per-file rules whose behaviour depends on the enclosing
/// package: runs a rule over every Dart file of `corpus/<rule>/project/` at its
/// real path (so `pubspec.yaml` discovery works) and checks the
/// `/* expect: <rule> */` annotations line by line. The in-process corpus runner
/// only exercises top-level fixtures for non-resolving rules.
#[cfg(test)]
pub(crate) fn assert_project_fixture(rule_name: &'static str, rule: &dyn falcon_analyze::Rule) {
    fn dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                dart_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "dart") {
                out.push(path);
            }
        }
    }

    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(rule_name)
        .join("project");
    let mut files = Vec::new();
    dart_files(&project, &mut files);
    let config = falcon_config::FalconConfig::default();
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(PathBuf::as_path),
        |_| None,
    );
    let marker = format!("/* expect: {rule_name} */");
    let mut expected_total = 0;
    let mut failures = Vec::new();
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let falcon_dart_parser::ParseResult { tree, .. } =
            falcon_dart_parser::parser::parse(&source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into_boxed_str(),
            tree,
        });
        let source = parsed.source.as_ref();
        let program = parsed.program();
        let ctx = falcon_analyze::AnalyzeContext::new(&path, &config, &parsed)
            .with_environment(Some(&environment));
        let line_of = |offset: usize| source[..offset].matches('\n').count() + 1;
        let diagnostics = if inference::requires_sdk(&path) {
            inference::analyze(rule_name, rule, &path, source, &config)
        } else {
            rule.analyze(rule_name, program, &ctx)
        };
        let mut actual: Vec<usize> = diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(
                    diagnostic.rule,
                    rule_name,
                    "diagnostic identity for {}",
                    path.display()
                );
                line_of(diagnostic.span.start)
            })
            .collect();
        actual.sort_unstable();
        let expected: Vec<usize> = source
            .lines()
            .enumerate()
            .flat_map(|(index, line)| std::iter::repeat_n(index + 1, line.matches(&marker).count()))
            .collect();
        expected_total += expected.len();
        if actual != expected {
            failures.push(format!(
                "{}: expected lines {expected:?}, got {actual:?}",
                path.display()
            ));
        }
    }
    assert!(
        expected_total > 0,
        "{rule_name} project fixture has no expectations"
    );
    assert!(
        failures.is_empty(),
        "{rule_name} project fixture mismatch:\n{}",
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parser::parse;

    #[test]
    fn enclosing_package_cache_tracks_manifest_changes() {
        let root = std::env::temp_dir().join(format!(
            "falcon-cached-package-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let manifest = root.join("pubspec.yaml");
        let source = root.join("lib/example.dart");
        std::fs::write(&manifest, "name: first\n").unwrap();
        assert_eq!(enclosing_package(&source).unwrap().name, "first");
        assert_eq!(lib_dir_package(&source).unwrap().name, "first");
        // A different length makes invalidation deterministic even on a coarse clock.
        std::fs::write(&manifest, "name: second_package\n").unwrap();
        assert_eq!(enclosing_package(&source).unwrap().name, "second_package");
        assert!(lib_dir_package(&root.join("test/example.dart")).is_none());
        std::fs::remove_file(&manifest).unwrap();
        assert!(enclosing_package(&source).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn missing_lib_root_agrees_for_saved_and_virtual_sources() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(package.join("bin")).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: app\n").unwrap();
        std::fs::write(package.join("bin/main.dart"), "void main() {}\n").unwrap();
        std::os::unix::fs::symlink(&package, &workspace).unwrap();
        let source = workspace.join("lib/src/unsaved.dart");
        let saved_package = enclosing_package(&workspace.join("bin/main.dart")).unwrap();
        let virtual_package = lib_dir_package(&source).unwrap();

        assert!(!workspace.join("lib").exists());
        assert_eq!(
            saved_package.lib_root,
            canonical_or_lexical(&package).join("lib")
        );
        assert_eq!(virtual_package.lib_root, saved_package.lib_root);
        assert!(canonical_package_path(&source).starts_with(&saved_package.lib_root));
    }

    #[cfg(unix)]
    #[test]
    fn lib_dir_package_recognizes_virtual_files_in_symlinked_workspace() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(package.join("lib")).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: app\n").unwrap();
        std::os::unix::fs::symlink(&package, &workspace).unwrap();

        let source = workspace.join("lib/src/unsaved.dart");
        assert!(!source.exists());
        let identity = lib_dir_package(&source).unwrap();
        assert_eq!(identity.name, "app");
        assert_eq!(
            identity.lib_root,
            canonical_or_lexical(&package.join("lib"))
        );
        assert!(lib_dir_package(&workspace.join("test/unsaved.dart")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn virtual_package_paths_resolve_lib_root_but_preserve_inner_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let sources = root.path().join("sources");
        let target = root.path().join("target");
        std::fs::create_dir(&package).unwrap();
        std::fs::create_dir(&sources).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: app\n").unwrap();
        std::os::unix::fs::symlink(&sources, package.join("lib")).unwrap();
        std::os::unix::fs::symlink(&target, sources.join("inner")).unwrap();
        let source = package.join("lib/inner/unsaved.dart");

        assert!(!source.exists());
        assert_eq!(
            canonical_package_path(&source),
            canonical_or_lexical(&sources).join("inner/unsaved.dart")
        );
        assert!(lib_dir_package(&source).is_some());

        std::fs::write(&source, "").unwrap();
        assert_eq!(
            canonical_package_path(&source),
            canonical_or_lexical(&target.join("unsaved.dart"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn enclosing_package_cache_preserves_symlink_roots() {
        let root = std::env::temp_dir().join(format!(
            "falcon-cached-symlinks-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = root.join("package");
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        std::fs::write(root.join("shared.yaml"), "name: shared\n").unwrap();
        std::os::unix::fs::symlink(root.join("shared.yaml"), package.join("pubspec.yaml")).unwrap();
        std::os::unix::fs::symlink(&first, package.join("lib")).unwrap();
        let source = package.join("example.dart");
        assert_eq!(
            enclosing_package(&source).unwrap().lib_root,
            canonical_or_lexical(&first)
        );
        std::fs::remove_file(package.join("lib")).unwrap();
        std::os::unix::fs::symlink(&second, package.join("lib")).unwrap();
        assert_eq!(
            enclosing_package(&source).unwrap().lib_root,
            canonical_or_lexical(&second)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn detect_package_keeps_symlinked_lib_identity() {
        let root = std::env::temp_dir().join(format!(
            "falcon-package-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package_root = root.join("package");
        let sources = root.join("sources");
        std::fs::create_dir_all(&package_root).unwrap();
        std::fs::create_dir(&sources).unwrap();
        std::fs::write(package_root.join("pubspec.yaml"), "name: 'pkg' # comment\n").unwrap();
        std::os::unix::fs::symlink(&sources, package_root.join("lib")).unwrap();
        let falcon_dart_parser::ParseResult { tree, errors } = parse("");
        assert!(errors.is_empty());
        let package = detect_package(&[ProjectFile {
            path: package_root.join("test.dart"),
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: "".into(),
                tree,
            }),
            has_parse_errors: false,
        }]);
        let expected_root = canonical_or_lexical(&sources);
        std::fs::remove_dir_all(root).unwrap();
        let package = package.unwrap();
        assert_eq!(package.name, "pkg");
        assert_eq!(package.lib_root, expected_root);
    }

    #[test]
    fn program_reference_uris_yields_configurable_uris() {
        let src = "export 'src/impl_stub.dart'\n    if (dart.library.io) 'src/impl_io.dart'\n    if (dart.library.html) 'src/impl_web.dart';";
        let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
        assert!(errors.is_empty(), "{errors:?}");
        let uris: Vec<_> = program_reference_uris(tree.program(src)).collect();
        assert!(
            uris.iter().any(|uri| uri == "src/impl_stub.dart"),
            "default uri: {uris:?}"
        );
        assert!(
            uris.iter().any(|uri| uri == "src/impl_io.dart"),
            "io branch: {uris:?}"
        );
        assert!(
            uris.iter().any(|uri| uri == "src/impl_web.dart"),
            "web branch: {uris:?}"
        );
    }

    #[test]
    fn conditional_import_uris_are_collected_too() {
        let src = "import 'src/stub.dart' if (dart.library.io) 'src/io.dart';";
        let falcon_dart_parser::ParseResult { tree, .. } = parse(src);
        let uris: Vec<_> = program_reference_uris(tree.program(src)).collect();
        assert!(
            uris.iter().any(|uri| uri == "src/io.dart"),
            "import branch: {uris:?}"
        );
    }

    #[test]
    fn has_lib_component_restricts_pubspec_less_scope() {
        assert!(has_lib_component(Path::new("/proj/lib/foo.dart")));
        assert!(has_lib_component(Path::new("/proj/lib/src/foo.dart")));
        assert!(!has_lib_component(Path::new("/proj/test/helpers.dart")));
        assert!(!has_lib_component(Path::new("/proj/bin/script.dart")));
        assert!(!has_lib_component(Path::new("/proj/tool/gen.dart")));
    }

    #[test]
    fn yaml_string_span_covers_quoted_and_block_scalars() {
        let source = "quoted: 'BadName'\nblock: |\n  Bad-Block\nnext: value\n";
        let root: Value = serde_yaml::from_str(source).unwrap();
        let mapping = root.as_mapping().unwrap();
        for (key, expected) in [("quoted", "'BadName'"), ("block", "|\n  Bad-Block")] {
            let value = mapping.get(Value::String(key.to_string())).unwrap();
            let span = yaml_string_span(source, &root, value).unwrap();
            assert_eq!(&source[span.start..span.end], expected);
        }
    }

    #[test]
    fn yaml_string_span_uses_structured_path_instead_of_matching_text() {
        let source = "# homepage: http://same.example\ndescription: http://same.example\nhomepage: http://same.example\n";
        let root: Value = serde_yaml::from_str(source).unwrap();
        let homepage = root
            .as_mapping()
            .unwrap()
            .get(Value::String("homepage".to_string()))
            .unwrap();
        let span = yaml_string_span(source, &root, homepage).unwrap();
        assert_eq!(&source[span.start..span.end], "http://same.example");
        assert_eq!(source[..span.start].lines().count(), 3);
    }

    #[test]
    fn yaml_string_span_covers_escaped_and_folded_scalars() {
        let source = "prefix: café\nescaped: \"http:\\/\\/example.com\"\nfolded: >\n  git://example.com/\n  repository.git\nnext: value\n";
        let root: Value = serde_yaml::from_str(source).unwrap();
        let mapping = root.as_mapping().unwrap();
        for (key, expected) in [
            ("escaped", "\"http:\\/\\/example.com\""),
            ("folded", ">\n  git://example.com/\n  repository.git"),
        ] {
            let value = mapping.get(Value::String(key.to_string())).unwrap();
            let span = yaml_string_span(source, &root, value).unwrap();
            assert_eq!(&source[span.start..span.end], expected);
        }
    }
}
