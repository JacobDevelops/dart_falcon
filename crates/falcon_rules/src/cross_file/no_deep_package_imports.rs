//! Prefer another package's public barrel over libraries it exports without restrictions.
//!
//! Checks default and conditional import URIs using the nearest package configuration
//! or packages in the analyzed workspace. The barrel is `lib/<package>.dart`.
//! Own-package imports are exempt; `src/` imports are left to `implementation-imports`.
//! Only unrestricted export paths within the dependency's library root establish
//! a violation; exports with combinators or conditional alternatives are skipped.
//! Missing, unreadable or malformed sources are skipped. Export traversal is cached
//! per package configuration and package, bounded to 512 libraries and 8 MiB of source.
//! Libraries beyond those limits cannot establish a violation. Analyzed source snapshots
//! take precedence over files on disk.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use falcon_analyze::{
    AnalysisEnvironment, CrossFileRule, PackageIdentity, ProjectFile, SemanticIndexes,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::cst::Program;

use super::{ResolvedRef, canonical_package_path, project_packages, resolve_directive_uri};

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
            .map(|file| environment.file_package(&file.path))
            .collect();
        let packages = project_packages(&file_packages);
        let sources: HashMap<_, _> = files
            .iter()
            .map(|file| (canonical_package_path(&file.path), file))
            .collect();
        let mut dependencies: HashMap<_, HashMap<String, Option<PackageBarrel>>> = HashMap::new();
        let mut diagnostics = Vec::new();
        for (file, own_package) in files.iter().zip(&file_packages) {
            let configured = environment.package_config(&file.path);
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
                            let root = canonical_package_path(root);
                            let path = root.join(format!("{name}.dart"));
                            let exported = export_closure(name, &root, &path, &sources);
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
                        canonical_package_path(&target)
                    } else {
                        let Some(decoded) = falcon_analyze::percent_decode(relative) else {
                            continue;
                        };
                        canonical_package_path(&barrel.root.join(decoded))
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
                            file.path.to_string_lossy().as_ref(),
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
    name: &str,
    root: &Path,
    barrel: &Path,
    sources: &HashMap<PathBuf, &ProjectFile>,
) -> HashSet<PathBuf> {
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
            if file.has_parse_errors || file.source().len() > remaining_bytes {
                continue;
            }
            remaining_bytes -= file.source().len();
            push_exports(file.program(), &path, &package, &mut pending);
            exported.insert(path);
        } else {
            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > remaining_bytes as u64 {
                continue;
            }
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            let mut source = String::new();
            if file
                .take(remaining_bytes as u64 + 1)
                .read_to_string(&mut source)
                .is_err()
            {
                continue;
            }
            if source.len() > remaining_bytes {
                continue;
            }
            remaining_bytes -= source.len();
            let parsed = falcon_dart_parser::parse(&source);
            if !parsed.errors.is_empty() {
                continue;
            }
            push_exports(parsed.tree.program(&source), &path, &package, &mut pending);
            exported.insert(path);
        }
    }
    exported
}

fn push_exports(
    program: Program<'_>,
    path: &Path,
    package: &PackageIdentity,
    pending: &mut Vec<PathBuf>,
) {
    for export in program.exports() {
        if export.combinators().next().is_some() || export.configurable_uris().next().is_some() {
            continue;
        }
        let value = export.uri().value();
        let Some(decoded) = falcon_analyze::percent_decode(&value) else {
            continue;
        };
        if let Some(ResolvedRef::Path(target)) =
            resolve_directive_uri(path, &decoded, Some(package), std::slice::from_ref(package))
            && target.starts_with(&package.lib_root)
        {
            pending.push(target);
        }
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
        let project_file = |path, source: &str| ProjectFile {
            path,
            parsed: Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: falcon_dart_parser::parse(source).tree,
            }),
            has_parse_errors: false,
        };
        let mut files = vec![project_file(
            root.join("main.dart"),
            &std::fs::read_to_string(root.join("main.dart")).unwrap(),
        )];
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(|file| file.path.as_path()),
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
    fn oversized_dependency_source_is_not_parsed() {
        let directory = tempfile::tempdir().unwrap();
        let root = canonical_package_path(directory.path());
        let barrel = root.join("other.dart");
        let target = root.join("a.dart");
        std::fs::write(&target, "class A {}").unwrap();
        std::fs::write(&barrel, "export 'a.dart';").unwrap();
        assert!(export_closure("other", &root, &barrel, &HashMap::new()).contains(&target));
        let source = format!("export 'a.dart'; /*{}*/", " ".repeat(8 * 1024 * 1024));
        std::fs::write(&barrel, source).unwrap();
        assert!(!export_closure("other", &root, &barrel, &HashMap::new()).contains(&target));
    }
}
