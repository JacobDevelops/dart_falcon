//! Require imported packages to be declared in `pubspec.yaml`.
//!
//! Imports from `lib/` and `bin/` must name a regular dependency. Imports from
//! development-only locations such as `test/` and `tool/` may also name a
//! dev-dependency. SDK imports and imports of the package itself are ignored.
//! Like the analyzer, only the primary URI is checked, not conditional alternatives.
//! When `undeclared-dependencies` is enabled for a file, that rule owns missing
//! declarations and dev-dependencies imported from `lib/`; this rule only adds
//! its stricter executable/build-hook dependency check, avoiding duplicate reports.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::{canonical_or_lexical, enclosing_pubspec};

#[derive(Default)]
pub struct DependOnReferencedPackages;

use crate::lint::support::package_manifests::{
    PackageManifest, imported_package, is_library_file, read_manifest,
};

impl CrossFileRule for DependOnReferencedPackages {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut manifests: HashMap<PathBuf, Option<PackageManifest>> = HashMap::new();
        let mut diagnostics = Vec::new();
        let replacement = crate::meta::meta_for("undeclared-dependencies");

        for file in files {
            let Some(pubspec) = enclosing_pubspec(&file.path) else {
                continue;
            };
            let manifest = manifests
                .entry(pubspec.clone())
                .or_insert_with(|| read_manifest(&pubspec));
            let Some(manifest) = manifest else {
                continue;
            };
            let regular_dependency_required = is_public_or_executable(&file.path, &pubspec);
            let replacement_enabled = replacement.is_some_and(|meta| {
                config
                    .resolve_cross_file_rule_for(
                        &file.path.to_string_lossy(),
                        meta.group.as_str(),
                        meta.name,
                        meta.recommended,
                    )
                    .is_some()
            });

            let import_uris = file
                .program()
                .imports()
                .map(falcon_syntax::cst::ImportDirective::uri);
            let export_uris = file
                .program()
                .exports()
                .map(falcon_syntax::cst::ExportDirective::uri);
            for uri in import_uris.chain(export_uris) {
                let value = uri.value();
                let Some(package) = imported_package(&value) else {
                    continue;
                };
                if package == "flutter_gen"
                    || manifest.name == package
                    || manifest.dependencies.contains(package)
                    || (!regular_dependency_required && manifest.dev_dependencies.contains(package))
                {
                    continue;
                }
                if replacement_enabled
                    && (!manifest.dev_dependencies.contains(package)
                        || is_library_file(&file.path, &pubspec))
                {
                    continue;
                }

                diagnostics.push(Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!(
                        "The imported package `{package}` isn't a dependency of the importing package."
                    ),
                    file.path.to_string_lossy().into_owned(),
                    uri.span(),
                ));
            }
        }

        diagnostics
    }
}

fn is_public_or_executable(file: &Path, pubspec: &Path) -> bool {
    let root = pubspec.parent().unwrap_or_else(|| Path::new(""));
    let file = canonical_or_lexical(file);
    let Ok(relative) = file.strip_prefix(root) else {
        return false;
    };
    relative
        .components()
        .next()
        .is_some_and(|component| matches!(component.as_os_str().to_str(), Some("lib" | "bin")))
        || matches!(
            relative.to_str(),
            Some("hook/build.dart" | "hook/link.dart")
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::support::package_manifests::parse_manifest;
    use falcon_analyze::CrossFileRuleAnalysis;

    #[cfg(unix)]
    #[test]
    fn virtual_public_paths_in_symlinked_workspace_are_not_dev_only() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let workspace = root.path().join("workspace");
        let sources = root.path().join("sources");
        std::fs::create_dir(&package).unwrap();
        std::fs::create_dir(&sources).unwrap();
        std::os::unix::fs::symlink(&sources, package.join("lib")).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: app\n").unwrap();
        std::os::unix::fs::symlink(&package, &workspace).unwrap();
        let manifest = workspace.join("pubspec.yaml");

        for path in [
            "lib/api.dart",
            "bin/main.dart",
            "hook/build.dart",
            "hook/link.dart",
        ] {
            assert!(
                is_public_or_executable(&workspace.join(path), &manifest),
                "{path}"
            );
        }
        assert!(!is_public_or_executable(
            &workspace.join("test/test.dart"),
            &manifest
        ));
    }

    #[test]
    fn extracts_only_package_imports() {
        assert_eq!(imported_package("package:http/http.dart"), Some("http"));
        assert_eq!(imported_package("package:foo"), None);
        assert_eq!(imported_package("package:foo/"), None);
        assert_eq!(imported_package("dart:io"), None);
        assert_eq!(imported_package("../local.dart"), None);
    }

    #[test]
    fn malformed_package_identity_skips_manifest() {
        assert!(parse_manifest("dependencies: {http: ^1.0.0}\n").is_none());
        assert!(parse_manifest("name: [not, a, string]\n").is_none());
        assert!(parse_manifest("name: pkg\ndependencies: [http]\n").is_none());
        assert!(parse_manifest("name: pkg\ndev_dependencies: oops\n").is_none());
        assert!(parse_manifest("name: pkg\ndependencies:\n").is_some());
    }

    #[test]
    fn hook_files_require_regular_dependencies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/depend-on-referenced-packages/cross-file/bad-hook");
        let files: Vec<ProjectFile> = ["hook/build.dart", "hook/link.dart"]
            .into_iter()
            .map(|relative| {
                let path = root.join(relative);
                let source = std::fs::read_to_string(&path).unwrap();
                let falcon_dart_parser::ParseResult { tree, errors } =
                    falcon_dart_parser::parse(&source);
                assert!(errors.is_empty());
                ProjectFile {
                    path,
                    parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                        source: source.into(),
                        tree,
                    }),
                    has_parse_errors: false,
                }
            })
            .collect();
        assert_eq!(
            DependOnReferencedPackages
                .analyze_project(
                    "depend-on-referenced-packages",
                    &falcon_analyze::AnalysisEnvironment::default(),
                    &files,
                    &FalconConfig::default(),
                    None,
                )
                .len(),
            2
        );
    }

    #[test]
    fn public_scope_is_lib_bin_and_hooks() {
        let pubspec = Path::new("/project/pubspec.yaml");
        assert!(is_public_or_executable(
            Path::new("/project/lib/api.dart"),
            pubspec
        ));
        assert!(is_public_or_executable(
            Path::new("/project/bin/tool.dart"),
            pubspec
        ));
        assert!(is_public_or_executable(
            Path::new("/project/hook/build.dart"),
            pubspec
        ));
        assert!(is_public_or_executable(
            Path::new("/project/hook/link.dart"),
            pubspec
        ));
        assert!(!is_public_or_executable(
            Path::new("/project/hook/helper.dart"),
            pubspec
        ));
        assert!(!is_public_or_executable(
            Path::new("/project/test/api_test.dart"),
            pubspec
        ));
    }
}
