//! Prefer another package's public barrel over libraries it exports without restrictions.
//!
//! Checks default and conditional import URIs using the nearest package configuration
//! or packages in the analyzed workspace. The barrel is `lib/<package>.dart`.
//! Own-package imports are exempt; `src/` imports are left to `implementation-imports`.
//! Only unrestricted export paths within the dependency's library root establish
//! a violation; exports with combinators or conditional alternatives are skipped.
//! Missing, oversized, or syntactically invalid sources are skipped. Unreadable or
//! non-UTF-8 sources make the dependent barrel unknown. Export traversal is cached
//! per package configuration and package, bounded to 512 libraries and 8 MiB of source.
//! Libraries beyond those limits cannot establish a violation. Analyzed source snapshots
//! take precedence over files on disk.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_analyze::{
    AnalysisEnvironment, CrossFileRule, PackageConfigInput, PackageIdentity, ProjectFile, SemanticIndexes,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::cst::Program;

use super::{ResolvedRef, project_packages, resolve_directive_uri};

#[derive(Default)]
pub struct NoDeepPackageImports;

struct PackageBarrel {
    root: PathBuf,
    path: PathBuf,
    exported: HashSet<PathBuf>,
}

impl CrossFileRule for NoDeepPackageImports {
    fn prepare_project(
        &self,
        environment: &AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let file_packages: Vec<_> = files
            .iter()
            .map(|file| environment.file_package(file.path()))
            .collect();
        let packages = project_packages(&file_packages);
        let sources: HashMap<_, _> = files
            .iter()
            .map(|file| (environment.canonical_path(file.path()), file))
            .collect();
        let mut dependencies: HashMap<_, HashMap<String, Option<PackageBarrel>>> = HashMap::new();
        let mut diagnostics = Vec::new();
        for (file, own_package) in files.iter().zip(&file_packages) {
            let configured = environment.package_config(file.path());
            if matches!(configured, PackageConfigInput::Unknown { config: None, .. }) {
                continue;
            }
            let configured = configured.available();
            let dependencies = dependencies
                .entry(configured.map(|config| config.path.as_path()))
                .or_default();
            for import in file.program().imports() {
                for uri in std::iter::once(import.uri()).chain(
                    import
                        .configurable_uris()
                        .map(falcon_syntax::cst::ConfigurableUri::uri),
                ) {
                    let value = uri.value();
                    let Some((name, relative)) = value
                        .strip_prefix("package:")
                        .and_then(|uri| uri.split_once('/'))
                    else {
                        continue;
                    };
                    if own_package
                        .identity()
                        .is_some_and(|package| package.name == name)
                        || relative.is_empty()
                        || relative.starts_with("src/")
                        || relative.strip_suffix(".dart") == Some(name)
                    {
                        continue;
                    }
                    if !dependencies.contains_key(name) {
                        let barrel = (|| {
                            let root = if let Some(config) = configured {
                                &config.package(name)?.lib_root
                            } else {
                                let mut candidates =
                                    packages.iter().filter(|package| package.name == name);
                                let package = candidates.next()?;
                                if candidates.next().is_some() {
                                    return None;
                                }
                                &package.lib_root
                            };
                            let root = environment.canonical_path(root);
                            let path = root.join(format!("{name}.dart"));
                            let exported =
                                export_closure(environment, name, &root, &path, &sources)?;
                            Some(PackageBarrel {
                                root,
                                path,
                                exported,
                            })
                        })();
                        dependencies.insert(name.to_owned(), barrel);
                    }
                    let Some(Some(barrel)) = dependencies.get(name) else {
                        continue;
                    };
                    let target = if let Some(config) = configured {
                        let Some(target) = config.resolve_package_uri(&value) else {
                            continue;
                        };
                        environment.canonical_path(&target)
                    } else {
                        let Some(decoded) = falcon_analyze::percent_decode(relative) else {
                            continue;
                        };
                        environment.canonical_path(&barrel.root.join(decoded))
                    };
                    let Ok(relative) = target.strip_prefix(&barrel.root) else {
                        continue;
                    };
                    if relative.starts_with("src") || target == barrel.path {
                        continue;
                    }
                    if barrel.exported.contains(&target) {
                        diagnostics.push(Diagnostic::new(
                            "",
                            Severity::Warning,
                            format!("Import `package:{name}/{name}.dart` instead of reaching into `{value}`."),
                            file.path().to_string_lossy().as_ref(),
                            uri.span(),
                        ));
                    }
                }
            }
        }
        diagnostics
    }
}

fn export_closure(
    environment: &AnalysisEnvironment,
    name: &str,
    root: &Path,
    barrel: &Path,
    sources: &HashMap<PathBuf, &ProjectFile>,
) -> Option<HashSet<PathBuf>> {
    let package = PackageIdentity {
        name: name.to_owned(),
        lib_root: root.to_owned(),
    };
    let mut pending = vec![barrel.to_owned()];
    let mut visited = HashSet::new();
    let mut exported = HashSet::new();
    let mut remaining_bytes = 8 * 1024 * 1024;
    while let Some(path) = pending.pop() {
        if visited.len() >= 512 {
            break;
        }
        if !visited.insert(path.clone()) {
            continue;
        }
        if let Some(file) = sources.get(&path) {
            if file.has_parse_errors() || file.source().len() > remaining_bytes {
                continue;
            }
            remaining_bytes -= file.source().len();
            push_exports(environment, file.program(), &path, &package, &mut pending);
            exported.insert(path);
        } else {
            let source = match environment.dependency_source(&path, remaining_bytes) {
                falcon_analyze::DependencyInput::Present(source) => source,
                falcon_analyze::DependencyInput::Missing => continue,
                falcon_analyze::DependencyInput::Unknown(_) => return None,
            };
            if source.byte_len > remaining_bytes {
                continue;
            }
            remaining_bytes -= source.byte_len;
            for export in &source.exports {
                if !export.has_combinators && !export.has_configurable_uris {
                    push_export(environment, &export.uri, &path, &package, &mut pending);
                }
            }
            exported.insert(path);
        }
    }
    Some(exported)
}

fn push_exports(
    environment: &AnalysisEnvironment,
    program: Program<'_>,
    path: &Path,
    package: &PackageIdentity,
    pending: &mut Vec<PathBuf>,
) {
    for export in program.exports() {
        if export.combinators().next().is_some() || export.configurable_uris().next().is_some() {
            continue;
        }
        push_export(environment, &export.uri().value(), path, package, pending);
    }
}

fn push_export(
    environment: &AnalysisEnvironment,
    value: &str,
    path: &Path,
    package: &PackageIdentity,
    pending: &mut Vec<PathBuf>,
) {
    let Some(decoded) = falcon_analyze::percent_decode(value) else {
        return;
    };
    if let Some(ResolvedRef::Path(target)) = resolve_directive_uri(
        environment,
        path,
        &decoded,
        Some(package),
        std::slice::from_ref(package),
    ) && target.starts_with(&package.lib_root)
    {
        pending.push(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn dependency_disk_loading_respects_analyzed_snapshots() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/no-deep-package-imports/cross-file/bad");
        let project_file = |path, source: &str| {
            ProjectFile::new(path, Arc::new(falcon_analyze::ParsedFile::parse(source)))
                .expect("parsed fixture must be accepted by the project")
        };
        let mut files = vec![project_file(
            root.join("main.dart"),
            &std::fs::read_to_string(root.join("main.dart")).unwrap(),
        )];
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(falcon_analyze::ProjectFile::path),
            |_| None,
        );
        let analyze = |files: &[ProjectFile]| {
            NoDeepPackageImports.prepare_project(
                &environment,
                files,
                &FalconConfig::default(),
                None,
            )
        };
        assert_eq!(analyze(&files).len(), 6);
        files.push(project_file(root.join("packages/other/lib/other.dart"), ""));
        assert!(analyze(&files).is_empty());
    }

    #[test]
    fn unknown_configuration_does_not_fall_back_to_discovered_packages() {
        let directory = tempfile::tempdir().unwrap();
        let root = falcon_analyze::canonical_package_path(directory.path());
        std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(root.join("dep/lib")).unwrap();
        std::fs::write(root.join("pubspec.yaml"), "name: app\n").unwrap();
        std::fs::write(root.join("dep/pubspec.yaml"), "name: dep\n").unwrap();
        std::fs::write(root.join("dep/lib/dep.dart"), "export 'value.dart';").unwrap();
        std::fs::write(root.join("dep/lib/value.dart"), "class Value {}").unwrap();
        let configuration = root.join(".dart_tool/package_config.json");
        let project_file = |path, source: &str| {
            ProjectFile::new(path, Arc::new(falcon_analyze::ParsedFile::parse(source))).unwrap()
        };
        let files = [
            project_file(root.join("main.dart"), "import 'package:dep/value.dart';"),
            project_file(root.join("dep/lib/value.dart"), "class Value {}"),
        ];
        for (contents, expected) in [
            ("{", 0),
            (r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../dep/","packageUri":"lib/"},{"name":"broken"}]}"#, 1),
            (r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../dep/","packageUri":"lib/"}]}"#, 1),
        ] {
            std::fs::write(&configuration, contents).unwrap();
            let environment = AnalysisEnvironment::discover_with_sdk_locator(
                files.iter().map(ProjectFile::path), |_| None,
            );
            let diagnostics = NoDeepPackageImports.prepare_project(
                &environment, &files, &FalconConfig::default(), None,
            );
            assert_eq!(diagnostics.len(), expected, "{contents}: {diagnostics:?}");
            if expected == 1 {
                assert_eq!(diagnostics[0].file_path, files[0].path().to_string_lossy());
            }
        }
    }

    #[test]
    fn oversized_dependency_source_is_not_parsed() {
        let directory = tempfile::tempdir().unwrap();
        let root = falcon_analyze::canonical_package_path(directory.path());
        let barrel = root.join("other.dart");
        let target = root.join("a.dart");
        std::fs::write(&target, "class A {}").unwrap();
        std::fs::write(&barrel, "export 'a.dart';").unwrap();
        assert!(
            export_closure(
                &AnalysisEnvironment::default(),
                "other",
                &root,
                &barrel,
                &HashMap::new()
            )
            .unwrap()
            .contains(&target)
        );
        let source = format!("export 'a.dart'; /*{}*/", " ".repeat(8 * 1024 * 1024));
        std::fs::write(&barrel, source).unwrap();
        assert!(
            export_closure(
                &AnalysisEnvironment::default(),
                "other",
                &root,
                &barrel,
                &HashMap::new()
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn unreadable_dependency_source_is_unknown_and_warning_rearms_after_recovery() {
        let root = tempfile::tempdir().unwrap();
        let barrel = root.path().join("barrel.dart");
        let target = root.path().join("value.dart");
        std::fs::write(&barrel, "export 'value.dart';").unwrap();
        std::fs::write(&target, [0xff]).unwrap();
        let environment = AnalysisEnvironment::default();
        for expected in [1, 0] {
            environment.refresh_dependency_inputs(std::slice::from_ref(&target));
            assert!(
                export_closure(&environment, "other", root.path(), &barrel, &HashMap::new())
                    .is_none()
            );
            assert_eq!(environment.take_warnings().len(), expected);
        }
        std::fs::write(&target, "class Value {}").unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&target));
        assert!(
            export_closure(&environment, "other", root.path(), &barrel, &HashMap::new())
                .unwrap()
                .contains(&environment.canonical_path(&target))
        );
        std::fs::write(&target, [0xff]).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&target));
        assert!(
            export_closure(&environment, "other", root.path(), &barrel, &HashMap::new()).is_none()
        );
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[test]
    fn oversized_and_parse_error_siblings_do_not_hide_small_exports() {
        let directory = tempfile::tempdir().unwrap();
        let root = falcon_analyze::canonical_package_path(directory.path());
        let barrel = root.join("other.dart");
        let target = root.join("a.dart");
        std::fs::write(&target, "class A {}").unwrap();
        std::fs::write(
            &barrel,
            "export 'a.dart'; export 'huge.dart'; export 'broken.dart';",
        )
        .unwrap();
        std::fs::write(
            root.join("huge.dart"),
            format!("/*{}*/", " ".repeat(9 * 1024 * 1024)),
        )
        .unwrap();
        std::fs::write(root.join("broken.dart"), "class {").unwrap();
        let environment = AnalysisEnvironment::default();
        let closure =
            export_closure(&environment, "other", &root, &barrel, &HashMap::new()).unwrap();
        assert!(closure.contains(&target));
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }
}
