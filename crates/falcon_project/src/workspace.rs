//! Discover pub and Melos workspace metadata without invoking Dart tooling.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_analyze::PackageMetadata;
use falcon_config::PathFilter;
use glob::{MatchOptions, Pattern};
use serde_yaml::Value;

use crate::discover_project;

/// The nearest declared workspace and its member packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Canonical workspace directory.
    pub root: PathBuf,
    /// Named root and members ordered by canonical directory, using pubspec names.
    pub packages: Vec<WorkspacePackage>,
}

/// A package admitted by a workspace's member declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePackage {
    pub name: String,
    /// Canonical package directory, contained within the workspace root.
    pub root: PathBuf,
}

/// Find the nearest independent pub or Melos workspace ancestor.
///
/// Pub members must opt into `resolution: workspace`; their own workspace lists
/// are expanded recursively. Melos 7 uses pub membership, while legacy Melos
/// uses its `packages` globs and `ignore` patterns. Melos 7 script-selection
/// ignores do not change pub membership. Invalid metadata warns and is skipped.
/// All returned package roots are canonical and contained within the workspace root.
///
/// # Panics
///
/// Panics if an internally discovered package root is outside its workspace.
pub fn discover_workspace(start_dir: &Path) -> Option<Workspace> {
    let start = start_dir.canonicalize().ok()?;
    for root in start.ancestors() {
        let pubspec = read_yaml(&root.join("pubspec.yaml"));
        if pubspec
            .as_ref()
            .and_then(|value| value.get("resolution"))
            .and_then(Value::as_str)
            == Some("workspace")
        {
            continue;
        }
        let legacy_melos = read_yaml(&root.join("melos.yaml"));
        let pub_members = pubspec.as_ref().and_then(|value| value.get("workspace"));
        let embedded_melos = pubspec.as_ref().and_then(|value| value.get("melos"));
        let is_pub = pub_members.is_some_and(Value::is_sequence);
        if !is_pub
            && !legacy_melos.as_ref().is_some_and(Value::is_mapping)
            && !embedded_melos.is_some_and(Value::is_mapping)
        {
            continue;
        }
        let mut packages = BTreeMap::new();
        let include_root = !is_pub
            && legacy_melos.as_ref().is_some_and(|value| {
                value.get("useRootAsPackage").and_then(Value::as_bool) == Some(true)
            });
        if include_root {
            insert_package(root, &root.join("pubspec.yaml"), &mut packages);
        }
        if let Some(melos) = legacy_melos.as_ref() {
            for manifest in member_manifests(root, melos.get("packages")) {
                if read_yaml(&manifest).is_some() {
                    insert_package(root, &manifest, &mut packages);
                }
            }
        }
        let ignores = patterns(legacy_melos.as_ref().and_then(|value| value.get("ignore")));
        packages.retain(|directory, _| {
            let relative = directory.strip_prefix(root).unwrap();
            !matches_member(&ignores, relative)
        });
        if is_pub {
            insert_package(root, &root.join("pubspec.yaml"), &mut packages);
            let mut visited = HashSet::new();
            collect_pub_members(root, root, pub_members, &mut visited, &mut packages);
        }
        return Some(Workspace {
            root: root.to_path_buf(),
            packages: packages
                .into_iter()
                .map(|(root, name)| WorkspacePackage { name, root })
                .collect(),
        });
    }
    None
}

fn collect_pub_members(
    workspace_root: &Path,
    declaring_root: &Path,
    members: Option<&Value>,
    visited: &mut HashSet<PathBuf>,
    packages: &mut BTreeMap<PathBuf, String>,
) {
    for manifest in member_manifests(declaring_root, members) {
        let Some(metadata) = read_yaml(&manifest) else {
            continue;
        };
        if metadata.get("resolution").and_then(Value::as_str) != Some("workspace") {
            continue;
        }
        let Some(directory) = manifest.parent().and_then(|path| path.canonicalize().ok()) else {
            continue;
        };
        if !directory.starts_with(workspace_root) || !visited.insert(directory.clone()) {
            continue;
        }
        insert_package(workspace_root, &manifest, packages);
        collect_pub_members(
            workspace_root,
            &directory,
            metadata.get("workspace"),
            visited,
            packages,
        );
    }
}

fn insert_package(root: &Path, manifest: &Path, packages: &mut BTreeMap<PathBuf, String>) {
    let Some(directory) = manifest.parent().and_then(|path| path.canonicalize().ok()) else {
        return;
    };
    if !directory.starts_with(root) {
        return;
    }
    let metadata = PackageMetadata::load(manifest);
    if let Some(name) = metadata.parsed_name().filter(|name| !name.is_empty()) {
        packages.insert(directory, name.to_owned());
    }
}

fn read_yaml(path: &Path) -> Option<Value> {
    let source = match falcon_config::read_metadata_text(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read workspace metadata");
            return None;
        }
    };
    match serde_yaml::from_str(&source) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "invalid workspace metadata");
            None
        }
    }
}

fn patterns(value: Option<&Value>) -> Vec<Pattern> {
    value
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|raw| {
            let normalized = raw.trim_start_matches("./").trim_end_matches('/');
            match Pattern::new(normalized) {
                Ok(pattern) => Some(pattern),
                Err(error) => {
                    tracing::warn!(pattern = raw, %error, "invalid workspace member glob");
                    None
                }
            }
        })
        .collect()
}

fn member_manifests(root: &Path, members: Option<&Value>) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    for pattern in patterns(members) {
        let raw = pattern.as_str();
        if Pattern::escape(raw) == raw {
            let manifest = root.join(raw).join("pubspec.yaml");
            if manifest.is_file() {
                manifests.push(manifest);
            }
            continue;
        }
        // Only glob declarations need a walk, and only below their literal prefix.
        let prefix: PathBuf = Path::new(raw)
            .components()
            .take_while(|component| {
                let component = component.as_os_str().to_string_lossy();
                Pattern::escape(&component) == component
            })
            .collect();
        let start = root.join(prefix);
        let Ok(canonical_start) = start.canonicalize() else {
            continue;
        };
        if !canonical_start.starts_with(root) {
            continue;
        }
        let mut filter = PathFilter::from_includes(&[
            "!**/*.dart".into(),
            "!**/.git/**".into(),
            "!**/.dart_tool/**".into(),
        ]);
        filter.set_base_dir(root);
        let discovery = discover_project(&[start], &filter, true);
        manifests.extend(discovery.manifests.into_iter().filter(|manifest| {
            manifest
                .parent()
                .and_then(|directory| directory.strip_prefix(root).ok())
                .is_some_and(|relative| matches_member(std::slice::from_ref(&pattern), relative))
        }));
    }
    manifests.sort();
    manifests.dedup();
    manifests
}

fn matches_member(patterns: &[Pattern], relative: &Path) -> bool {
    let relative = if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    };
    patterns.iter().any(|pattern| {
        pattern.matches_path_with(
            relative,
            MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: true,
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, source: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }

    fn names(workspace: &Workspace) -> Vec<&str> {
        workspace
            .packages
            .iter()
            .map(|package| package.name.as_str())
            .collect()
    }

    #[test]
    fn pub_members_opt_in_and_use_their_own_names() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(
            root,
            "pubspec.yaml",
            "name: workspace_root\nworkspace: [packages/first, packages/not_opted_in, packages/missing, packages/broken]\n",
        );
        write(
            root,
            "packages/first/pubspec.yaml",
            "name: actual_package_name\nresolution: workspace\n",
        );
        write(
            root,
            "packages/not_opted_in/pubspec.yaml",
            "name: not_a_member\n",
        );
        write(
            root,
            "packages/broken/pubspec.yaml",
            "name: broken\nresolution: [\n",
        );
        write(
            root,
            "packages/unlisted/pubspec.yaml",
            "name: unlisted\nresolution: workspace\n",
        );
        let workspace = discover_workspace(&root.join("packages/first")).unwrap();
        assert_eq!(workspace.root, root.canonicalize().unwrap());
        assert_eq!(names(&workspace), ["workspace_root", "actual_package_name"]);
        assert_eq!(
            workspace.packages[1].root,
            root.join("packages/first").canonicalize().unwrap()
        );
    }

    #[test]
    fn melos_globs_respect_boundaries_ignore_and_root_opt_in() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(root, "pubspec.yaml", "name: repository\n");
        write(
            root,
            "melos.yaml",
            "packages: ['packages/*', 'plugins/**']\nignore: ['plugins/**/example']\n",
        );
        write(root, "packages/app/pubspec.yaml", "name: own_app_name\n");
        write(
            root,
            "packages/app/example/pubspec.yaml",
            "name: nested_nonmember\n",
        );
        write(root, "plugins/group/plugin/pubspec.yaml", "name: plugin\n");
        write(
            root,
            "plugins/group/plugin/example/pubspec.yaml",
            "name: ignored\n",
        );
        let workspace = discover_workspace(root).unwrap();
        assert_eq!(names(&workspace), ["own_app_name", "plugin"]);
        write(
            root,
            "melos.yaml",
            "packages: ['packages/*']\nuseRootAsPackage: true\n",
        );
        assert_eq!(
            names(&discover_workspace(root).unwrap()),
            ["repository", "own_app_name"]
        );
    }

    #[test]
    fn package_names_matching_build_directories_are_valid_glob_members() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(root, "pubspec.yaml", "workspace: ['packages/*']\n");
        for name in ["build", "node_modules", "Pods"] {
            write(
                root,
                &format!("packages/{name}/pubspec.yaml"),
                &format!("name: {name}\nresolution: workspace\n"),
            );
        }
        assert_eq!(
            names(&discover_workspace(root).unwrap()),
            ["Pods", "build", "node_modules"]
        );
    }

    #[test]
    fn melos_seven_script_ignores_do_not_change_pub_membership() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(
            root,
            "pubspec.yaml",
            "name: repository\nworkspace: [packages/pub, packages/ignored]\nmelos:\n  packages: ['packages/*']\n  ignore: ['packages/ignored']\n",
        );
        write(
            root,
            "packages/pub/pubspec.yaml",
            "name: pub_member\nresolution: workspace\n",
        );
        write(
            root,
            "packages/ignored/pubspec.yaml",
            "name: ignored\nresolution: workspace\n",
        );
        write(
            root,
            "packages/legacy/pubspec.yaml",
            "name: legacy_member\n",
        );
        assert_eq!(
            names(&discover_workspace(root).unwrap()),
            ["repository", "ignored", "pub_member"]
        );
    }

    #[test]
    fn nested_pub_members_expand_from_root_and_nested_start() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(root, "pubspec.yaml", "workspace: [server, sibling]\n");
        write(
            root,
            "server/pubspec.yaml",
            "name: server\nresolution: workspace\nworkspace: [auth]\n",
        );
        write(
            root,
            "server/auth/pubspec.yaml",
            "name: auth\nresolution: workspace\n",
        );
        write(
            root,
            "sibling/pubspec.yaml",
            "name: sibling\nresolution: workspace\n",
        );
        let workspace = discover_workspace(root).unwrap();
        assert_eq!(names(&workspace), ["server", "auth", "sibling"]);
        assert_eq!(
            discover_workspace(&root.join("server/auth")),
            Some(workspace)
        );
    }

    #[test]
    fn nearest_independent_workspace_ancestor_wins() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(root, "melos.yaml", "packages: ['**']\n");
        write(root, "nested/pubspec.yaml", "workspace: [member]\n");
        write(
            root,
            "nested/member/pubspec.yaml",
            "name: nearest_member\nresolution: workspace\n",
        );
        let workspace = discover_workspace(&root.join("nested/member")).unwrap();
        assert_eq!(workspace.root, root.join("nested").canonicalize().unwrap());
        assert_eq!(names(&workspace), ["nearest_member"]);
    }

    #[test]
    fn pub_and_legacy_melos_metadata_can_coexist() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(root, "pubspec.yaml", "workspace: [pub]\n");
        write(
            root,
            "pub/pubspec.yaml",
            "name: pub_member\nresolution: workspace\n",
        );
        write(root, "legacy/pubspec.yaml", "name: legacy_member\n");
        write(root, "melos.yaml", "packages: [legacy]\n");
        assert_eq!(
            names(&discover_workspace(root).unwrap()),
            ["legacy_member", "pub_member"]
        );
    }

    #[test]
    fn ordinary_packages_and_invalid_metadata_are_not_workspaces() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write(
            root,
            "pubspec.yaml",
            "name: standalone\nresolution: workspace\n",
        );
        write(root, "melos.yaml", "[invalid\n");
        assert!(discover_workspace(root).is_none());
        write(root, "pubspec.yaml", "workspace: []\n");
        assert_eq!(
            discover_workspace(root).unwrap().packages,
            [] as [WorkspacePackage; 0]
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonical_paths_deduplicate_aliases_and_reject_external_members() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        write(
            &root,
            "pubspec.yaml",
            "workspace: [packages/external, packages/member, packages/alias]\n",
        );
        write(
            directory.path(),
            "external/pubspec.yaml",
            "name: outside\nresolution: workspace\n",
        );
        write(
            &root,
            "packages/member/pubspec.yaml",
            "name: internal\nresolution: workspace\n",
        );
        std::os::unix::fs::symlink(root.join("packages/member"), root.join("packages/alias"))
            .unwrap();
        std::os::unix::fs::symlink(
            directory.path().join("external"),
            root.join("packages/external"),
        )
        .unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let workspace = discover_workspace(&alias).unwrap();
        assert_eq!(workspace.root, root.canonicalize().unwrap());
        assert_eq!(names(&workspace), ["internal"]);
        assert_eq!(
            workspace.packages[0].root,
            root.join("packages/member").canonicalize().unwrap()
        );
    }
    #[test]
    fn oversized_workspace_metadata_is_skipped_and_recovers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pubspec.yaml");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(falcon_config::MAX_METADATA_BYTES + 1)
            .unwrap();
        assert!(read_yaml(&path).is_none());
        std::fs::write(&path, "name: recovered\nworkspace: []\n").unwrap();
        assert!(discover_workspace(directory.path()).is_some());
    }
}
