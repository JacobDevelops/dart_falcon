//! Cross-file rules — falcon's port of dart_code_linter's `check-unused-files` /
//! `check-unused-code` commands plus a conservative `check-unnecessary-nullable`.
//! These run in the CLI and LSP cross-file passes (see
//! `falcon_analyze::CrossFileRule`).
//!
//! This module root holds the shared cross-file plumbing every rule needs:
//! package/pubspec discovery, directive-URI resolution, and path normalization.

pub mod unnecessary_nullable;
pub mod unused_code;
pub mod unused_files;

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use falcon_analyze::ProjectFile;
use falcon_syntax::ast::{ExportDirective, ImportDirective, PartDirective, Program};

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

/// Discover the enclosing package by walking up from the analyzed files to the
/// first `pubspec.yaml`, reading its `name:`. Tolerant: any read/parse failure
/// yields `None` and callers fall back to suffix matching.
pub fn detect_package(files: &[ProjectFile]) -> Option<PackageInfo> {
    for f in files {
        let canon = canonical_or_lexical(&f.path);
        let mut dir = canon.parent();
        while let Some(d) = dir {
            let pubspec = d.join("pubspec.yaml");
            if pubspec.exists() {
                return read_pubspec_name(&pubspec).map(|name| PackageInfo {
                    name,
                    lib_root: canonical_or_lexical(&d.join("lib")),
                });
            }
            dir = d.parent();
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
        if let Some(pkg) = pkg
            && pkg_name == pkg.name
        {
            return Some(ResolvedRef::Path(canonical_or_lexical(
                &pkg.lib_root.join(sub),
            )));
        }
        // Unknown package (or no pubspec): match on the `lib/<sub>` suffix.
        return Some(ResolvedRef::Suffix(format!("lib/{sub}")));
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
        assert!(uris.contains(&"src/impl_stub.dart"), "default uri: {uris:?}");
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
}
