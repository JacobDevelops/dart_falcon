//! Prefer relative imports for libraries of the same package under `lib/`.
//!
//! Adopted from the official `prefer_relative_imports` lint. The rule runs only
//! for files inside the enclosing package's `lib/` directory (the nearest
//! `pubspec.yaml` names the package; files outside any package, or in a package
//! without a `name`, are skipped). An `import` whose URI is a `package:` URI
//! naming the file's own package, such as `import 'package:my_pkg/src/a.dart';`
//! inside `my_pkg/lib/`, is reported and should be written relative
//! (`import 'src/a.dart';`). Imports of other packages, `dart:` libraries,
//! exports, and conditional alternatives are not checked. Incompatible with
//! `always-use-package-imports`.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::correctness::always_use_package_imports::uri_scheme;
use crate::lint::string_lit::decode_escapes;

#[derive(Default)]
pub struct PreferRelativeImports;


impl Rule for PreferRelativeImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if program.imports().is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let Some(package) = crate::cross_file::lib_dir_package(ctx.file_path()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if package.name.is_empty() {
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
                    is_self_package_uri(&value, &package.name)
                })
                .map(|import| {
                    ctx.diagnostic("Use relative imports for files in the `lib` directory.", import.uri().span())
                })
                .collect(),
        )
    }
}

/// Whether `uri` is a well-formed `package:<package>/<path>` URI.
fn is_self_package_uri(uri: &str, package: &str) -> bool {
    if uri_scheme(uri) != Some("package") {
        return false;
    }
    uri["package:".len()..]
        .split_once('/')
        .is_some_and(|(name, path)| name == package && !path.is_empty())
}

#[cfg(test)]
mod tests {
    use super::PreferRelativeImports;
    use super::is_self_package_uri;

    #[test]
    fn matches_only_own_package_uris() {
        assert!(is_self_package_uri("package:app/src/a.dart", "app"));
        assert!(!is_self_package_uri("package:other/a.dart", "app"));
        assert!(!is_self_package_uri("package:app/", "app"));
        assert!(!is_self_package_uri("src/a.dart", "app"));
        assert!(!is_self_package_uri("package:application/a.dart", "app"));
    }

    #[test]
    fn project_fixture_matches_expectations() {
        crate::cross_file::assert_project_fixture(
            "prefer-relative-imports",
            &PreferRelativeImports,
        );
    }
}
