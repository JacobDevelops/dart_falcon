//! Disallow relative imports in libraries under a package's `lib/` directory.
//!
//! Adopted from the official `always_use_package_imports` lint. The rule runs
//! only for files inside the enclosing package's `lib/` directory (the nearest
//! `pubspec.yaml` decides the package; files outside any package are skipped,
//! like the analyzer's `isInLibDir`). Every `import` whose URI parses with an
//! empty scheme is reported, so `import 'src/a.dart';` must be spelled
//! `import 'package:<name>/src/a.dart';`. `package:` and `dart:` imports,
//! exports, and conditional alternatives are not checked; relative imports that
//! reach into `lib/` from outside it belong to `avoid_relative_lib_imports`.
//! Incompatible with `prefer-relative-imports`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::lint::string_lit::decode_escapes;

#[derive(Default)]
pub struct AlwaysUsePackageImports;

const NAME: &str = "always-use-package-imports";

impl Rule for AlwaysUsePackageImports {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if program.imports.is_empty() || crate::cross_file::lib_dir_package(ctx.file_path).is_none()
        {
            return Vec::new();
        }
        program
            .imports
            .iter()
            .filter(|import| {
                let uri = &import.uri;
                let value = if uri.has_raw_segment {
                    uri.value.as_str().into()
                } else {
                    decode_escapes(&uri.value)
                };
                uri_scheme(&value) == Some("")
            })
            .map(|import| {
                ctx.diagnostic(
                    NAME,
                    "Use 'package:' imports for files in the 'lib' directory.",
                    import.uri.span,
                )
            })
            .collect()
    }
}

/// The scheme of `uri` as Dart's `Uri.tryParse` sees it: `Some("")` for a
/// scheme-less (relative) reference, `Some(scheme)` for an absolute URI, and
/// `None` when the text is not a parseable URI.
pub(crate) fn uri_scheme(uri: &str) -> Option<&str> {
    let end = uri.find(['/', '?', '#']).unwrap_or(uri.len());
    let scheme = match uri[..end].find(':') {
        Some(colon) => {
            let scheme = &uri[..colon];
            let mut chars = scheme.chars();
            let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
            if !valid {
                return None;
            }
            scheme
        }
        None => "",
    };
    valid_percent_escapes(uri).then_some(scheme)
}

fn valid_percent_escapes(uri: &str) -> bool {
    let bytes = uri.as_bytes();
    bytes.iter().enumerate().all(|(index, &byte)| {
        byte != b'%'
            || bytes
                .get(index + 1..index + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
    })
}

#[cfg(test)]
mod tests {
    use super::AlwaysUsePackageImports;
    use super::uri_scheme;

    #[test]
    fn classifies_uri_schemes_like_dart_uri_parse() {
        assert_eq!(uri_scheme("src/a.dart"), Some(""));
        assert_eq!(uri_scheme("../b.dart"), Some(""));
        assert_eq!(uri_scheme("a.dart?x=y:z"), Some(""));
        assert_eq!(uri_scheme("package:pkg/a.dart"), Some("package"));
        assert_eq!(uri_scheme("dart:core"), Some("dart"));
        assert_eq!(uri_scheme("1pkg:a.dart"), None);
        assert_eq!(uri_scheme("a%zz.dart"), None);
        assert_eq!(uri_scheme("a%20b.dart"), Some(""));
    }

    #[test]
    fn project_fixture_matches_expectations() {
        crate::cross_file::assert_project_fixture(
            "always-use-package-imports",
            &AlwaysUsePackageImports,
        );
    }
}
