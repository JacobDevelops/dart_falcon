//! Project-level (cross-file) rules — falcon's port of dart_code_linter's
//! `check-unused-files` / `check-unused-code` commands plus a conservative
//! `check-unnecessary-nullable`. These run in the CLI's project pass (see
//! `falcon_analyze::ProjectRule`); the single-file LSP model never runs them.
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
/// and `part` directives (the things that reference another file).
fn program_reference_uris(program: &Program) -> impl Iterator<Item = &str> {
    program
        .imports
        .iter()
        .map(|i: &ImportDirective| i.uri.value.as_str())
        .chain(
            program
                .exports
                .iter()
                .map(|e: &ExportDirective| e.uri.value.as_str()),
        )
        .chain(
            program
                .part_directives
                .iter()
                .map(|p: &PartDirective| p.uri.value.as_str()),
        )
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
/// without pubspec metadata (rare in real projects, common in tests) every
/// analyzed file is treated as in-scope.
pub fn is_under_lib(path: &Path, pkg: Option<&PackageInfo>) -> bool {
    match pkg {
        Some(pkg) => path.starts_with(&pkg.lib_root),
        None => true,
    }
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
