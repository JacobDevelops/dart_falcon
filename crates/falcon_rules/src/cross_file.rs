//! Cross-file rules — falcon's port of dart_code_linter's `check-unused-files` /
//! `check-unused-code` commands plus a conservative `check-unnecessary-nullable`.
//! These run in the CLI and LSP cross-file passes (see
//! `falcon_analyze::CrossFileRule`).
//!
//! This module root holds the shared cross-file plumbing every rule needs:
//! package/pubspec discovery, directive-URI resolution, and path normalization.

pub mod depend_on_referenced_packages;
pub mod package_names;
pub mod secure_pubspec_urls;
pub mod unnecessary_nullable;
pub mod unused_code;
pub mod unused_files;

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use falcon_analyze::ProjectFile;
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::ast::{ExportDirective, ImportDirective, PartDirective, Program};
use marked_yaml::{Node as MarkedNode, parse_yaml};
use serde_yaml::Value;

/// Resolved package identity: the `name:` from pubspec.yaml and the absolute
/// `lib/` directory that `package:<name>/...` URIs resolve against.
pub struct PackageInfo {
    pub name: String,
    pub lib_root: PathBuf,
}

/// A resolved reference target: either a concrete file path or, when the target
/// package/path can't be pinned to an analyzed file, a `lib/...` path suffix to
/// match tolerantly.
pub enum ResolvedRef {
    Path(PathBuf),
    Suffix(String),
}

/// Lexically normalize a path (resolve `.`/`..` without touching the FS).
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Canonical path when the file exists (resolving symlinks so two spellings of
/// the same file compare equal), falling back to lexical normalization.
pub fn canonical_or_lexical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| lexical_normalize(path))
}

/// Forward-slash string form of a path, for suffix comparisons.
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

/// Discover the enclosing package by walking up from the analyzed files to the
/// first `pubspec.yaml`, reading its `name:`. Tolerant: any read/parse failure
/// yields `None` and callers fall back to suffix matching.
pub fn detect_package(files: &[ProjectFile]) -> Option<PackageInfo> {
    for f in files {
        if let Some(pubspec) = enclosing_pubspec(&f.path) {
            let root = pubspec.parent().unwrap_or_else(|| Path::new(""));
            return read_pubspec_name(&pubspec).map(|name| PackageInfo {
                name,
                lib_root: canonical_or_lexical(&root.join("lib")),
            });
        }
    }
    None
}

/// Extract the top-level `name:` value from a pubspec.yaml (first match wins).
fn read_pubspec_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        // Only a top-level `name:` (no indentation) is the package name.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name:") {
            let name = rest
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Resolve an `import`/`export`/`part` URI (already the string value, without
/// quotes) relative to `from_file`. `dart:` URIs resolve to `None` (external).
pub fn resolve_directive_uri(
    from_file: &Path,
    uri: &str,
    pkg: Option<&PackageInfo>,
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
    // Relative URI, resolved against the importing file's directory.
    let from_dir = from_file.parent().unwrap_or_else(|| Path::new(""));
    Some(ResolvedRef::Path(canonical_or_lexical(&from_dir.join(uri))))
}

/// All directive URIs a program declares, as `(uri_value)` — imports, exports,
/// and `part` directives (the things that reference another file). Each
/// import/export also yields its `configurable_uris` targets, so conditional
/// (`if (dart.library.io) '...'`) platform-specific files count as referenced.
fn program_reference_uris(program: &Program) -> impl Iterator<Item = &str> {
    let imports = program.imports.iter().flat_map(|i: &ImportDirective| {
        std::iter::once(i.uri.value.as_str())
            .chain(i.configurable_uris.iter().map(|c| c.uri.value.as_str()))
    });
    let exports = program.exports.iter().flat_map(|e: &ExportDirective| {
        std::iter::once(e.uri.value.as_str())
            .chain(e.configurable_uris.iter().map(|c| c.uri.value.as_str()))
    });
    let parts = program
        .part_directives
        .iter()
        .map(|p: &PartDirective| p.uri.value.as_str());
    imports.chain(exports).chain(parts)
}

/// A set of resolved reference targets: concrete canonical paths plus tolerant
/// path suffixes. Used to answer "is this file referenced by anything?".
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

    /// Whether `path` (a canonical file path) is one of the referenced targets.
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
pub fn is_under_lib(path: &Path, pkg: Option<&PackageInfo>) -> bool {
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
    pkg: Option<&PackageInfo>,
) -> Vec<Vec<usize>> {
    let mut by_path: HashMap<&Path, usize> = HashMap::with_capacity(canons.len());
    for (i, c) in canons.iter().enumerate() {
        by_path.entry(c.as_path()).or_insert(i);
    }
    let slashed_paths: Vec<String> = canons.iter().map(|c| slashed(c)).collect();

    let mut adj = vec![Vec::new(); files.len()];
    for (i, f) in files.iter().enumerate() {
        for uri in program_reference_uris(&f.program) {
            match resolve_directive_uri(&canons[i], uri, pkg) {
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

/// Collect every file's resolved outgoing references into one [`ReferenceSet`].
pub fn collect_references(files: &[ProjectFile], pkg: Option<&PackageInfo>) -> ReferenceSet {
    let mut refs = ReferenceSet::default();
    for f in files {
        let from = canonical_or_lexical(&f.path);
        for uri in program_reference_uris(&f.program) {
            if let Some(r) = resolve_directive_uri(&from, uri, pkg) {
                refs.insert(r);
            }
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parser::parse;

    fn project_file(path: &str, source: &str) -> ProjectFile {
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "fixture must parse: {errors:?}");
        ProjectFile {
            path: PathBuf::from(path),
            source: source.to_string(),
            program,
            has_parse_errors: false,
        }
    }

    // Finding: conditional import/export URIs were ignored, so platform-specific
    // impl files were falsely reported unused.
    #[test]
    fn program_reference_uris_yields_configurable_uris() {
        let src = "export 'src/impl_stub.dart'\n    if (dart.library.io) 'src/impl_io.dart'\n    if (dart.library.html) 'src/impl_web.dart';";
        let (program, errors) = parse(src);
        assert!(errors.is_empty(), "{errors:?}");
        let uris: Vec<&str> = program_reference_uris(&program).collect();
        assert!(
            uris.contains(&"src/impl_stub.dart"),
            "default uri: {uris:?}"
        );
        assert!(uris.contains(&"src/impl_io.dart"), "io branch: {uris:?}");
        assert!(uris.contains(&"src/impl_web.dart"), "web branch: {uris:?}");
    }

    #[test]
    fn conditional_import_uris_are_collected_too() {
        let src = "import 'src/stub.dart' if (dart.library.io) 'src/io.dart';";
        let (program, _) = parse(src);
        let uris: Vec<&str> = program_reference_uris(&program).collect();
        assert!(uris.contains(&"src/io.dart"), "import branch: {uris:?}");
    }

    #[test]
    fn collect_references_resolves_conditional_impl_files() {
        let f = project_file(
            "/pkg/lib/api.dart",
            "export 'src/impl_stub.dart' if (dart.library.io) 'src/impl_io.dart' if (dart.library.html) 'src/impl_web.dart';",
        );
        let refs = collect_references(std::slice::from_ref(&f), None);
        for target in ["impl_stub", "impl_io", "impl_web"] {
            let p = PathBuf::from(format!("/pkg/lib/src/{target}.dart"));
            assert!(refs.contains(&p), "{target} should be referenced");
        }
    }

    // Finding: with no pubspec, unused-files flagged test/bin/tool files. The
    // pubspec-less scope fallback must accept only paths with a `lib/` component.
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
