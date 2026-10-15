//! Flags a library name that is not prefixed by its package name and path.
//!
//! Named libraries share one global namespace, so Effective Dart builds each
//! name from the package name (from the enclosing `pubspec.yaml`) followed by
//! the dot-separated directory path of the file, with a leading `lib/` dropped.
//! The name must equal that prefix or extend it with further dotted segments:
//!
//! ```dart
//! // my_package/lib/src/parser/scanner.dart
//! library my_package.src.parser.scanner;  // fine
//! library my_package.src.parser;          // fine: equals the prefix
//! library scanner;                        // flagged
//! ```
//!
//! Files with no enclosing `pubspec.yaml`, or whose manifest has no `name`, are
//! not checked. As in the upstream `createLibraryNamePrefix`, a file directly
//! in the package root gets the prefix `<package>..`, so its named library is
//! always reported.
//!
//! UPSTREAM-NOOP: the official `package_prefixed_library_names` lint returns
//! immediately because the analyzer no longer provides project information
//! (dart-lang/linter#3395). Falcon implements its documented, formerly shipped
//! behaviour.

use std::path::{Component, Path};

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PackagePrefixedLibraryNames;

impl Rule for PackagePrefixedLibraryNames {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(lib) = &program.library_directive else {
            return Vec::new();
        };
        let (Some(first), Some(last)) = (lib.name.first(), lib.name.last()) else {
            return Vec::new();
        };
        let Some(prefix) = library_name_prefix(ctx.file_path) else {
            return Vec::new();
        };
        let name = lib
            .name
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let prefixed = name
            .strip_prefix(prefix.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
        if prefixed {
            return Vec::new();
        }
        vec![ctx.diagnostic(
            "package-prefixed-library-names",
            format!(
                "The library name is not a dot-separated path prefixed by the package name. \
                 Try changing the name to '{prefix}.{name}'."
            ),
            DiagSpan {
                start: first.span.start,
                end: last.span.end,
            },
        )]
    }
}

/// The analyzer's `createLibraryNamePrefix`: the package name followed by the
/// file's directory relative to the package root, minus a leading `lib`.
fn library_name_prefix(path: &Path) -> Option<String> {
    let pubspec = crate::cross_file::enclosing_pubspec(path)?;
    let root = pubspec.parent()?;
    let file = falcon_analyze::canonical_or_lexical(path);
    let directory = file.parent()?.strip_prefix(root).ok()?;
    let source = std::fs::read_to_string(&pubspec).ok()?;
    let manifest: serde_yaml::Value = serde_yaml::from_str(&source).ok()?;
    let package = manifest.get("name")?.as_str()?;

    let mut prefix = package.to_string();
    let mut segments = directory
        .components()
        .filter_map(|component| match component {
            Component::Normal(segment) => segment.to_str(),
            _ => None,
        });
    let mut first = segments.next();
    if first.is_none() {
        // `path.relative` yields `.` for the root, which becomes `..`.
        prefix.push_str("..");
        return Some(prefix);
    }
    if first == Some("lib") {
        first = segments.next();
    }
    for segment in first.into_iter().chain(segments) {
        prefix.push('.');
        prefix.push_str(segment);
    }
    Some(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    fn diagnostic_lines(path: &Path) -> Vec<usize> {
        let source = std::fs::read_to_string(path).unwrap();
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = parse(&source);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &source, &config, &syntax);
        PackagePrefixedLibraryNames
            .analyze(&program, &ctx)
            .into_iter()
            .map(|diag| source[..diag.span.start].matches('\n').count() + 1)
            .collect()
    }

    fn expected_lines(path: &Path) -> Vec<usize> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains("/* expect: package-prefixed-library-names */"))
            .map(|(index, _)| index + 1)
            .collect()
    }

    /// The corpus project fixtures need their `pubspec.yaml`, so they are
    /// checked here rather than by the top-level corpus runner.
    #[test]
    fn project_fixtures_match_expectations() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/package-prefixed-library-names/project");
        for file in [
            "lib/bad_root.dart",
            "lib/src/bad_nested.dart",
            "lib/good_root.dart",
            "lib/src/parser/good_nested.dart",
            "test/good_test_dir.dart",
            "tool/bad_tool.dart",
            "bad_package_root.dart",
        ] {
            let path = project.join(file);
            assert_eq!(diagnostic_lines(&path), expected_lines(&path), "{file}");
        }
    }

    #[test]
    fn prefix_drops_leading_lib_only() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/package-prefixed-library-names/project");
        assert_eq!(
            library_name_prefix(&project.join("lib/src/parser/good_nested.dart")).as_deref(),
            Some("prefixed_pkg.src.parser")
        );
        assert_eq!(
            library_name_prefix(&project.join("lib/good_root.dart")).as_deref(),
            Some("prefixed_pkg")
        );
        assert_eq!(
            library_name_prefix(&project.join("test/good_test_dir.dart")).as_deref(),
            Some("prefixed_pkg.test")
        );
        assert_eq!(
            library_name_prefix(&project.join("bad_package_root.dart")).as_deref(),
            Some("prefixed_pkg..")
        );
    }
}
