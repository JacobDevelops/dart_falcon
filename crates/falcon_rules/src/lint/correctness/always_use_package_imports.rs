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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::string_lit::decode_escapes;
use crate::lint::support::uri::uri_scheme;

#[derive(Default)]
pub struct AlwaysUsePackageImports;


impl Rule for AlwaysUsePackageImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if program.imports().is_empty()
            || ctx.lib_dir_package().is_none()
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Diagnostics(
            program
                .imports()
                .filter(|import| {
                    let uri = import.uri();
                    let raw_value = uri.value();
                    let value = if uri.has_raw_segment() {
                        raw_value
                    } else {
                        decode_escapes(&raw_value)
                    };
                    uri_scheme(&value) == Some("")
                })
                .map(|import| {
                    ctx.diagnostic("Use `package:` imports for files in the `lib` directory.", import.uri().span())
                })
                .collect(),
        )
    }
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
