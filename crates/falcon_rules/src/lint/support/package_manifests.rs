use falcon_analyze::canonical_package_path;
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct PackageManifest {
    pub(crate) name: String,
    pub(crate) dependencies: HashSet<String>,
    pub(crate) dev_dependencies: HashSet<String>,
}

pub(crate) fn read_manifest(path: &Path) -> Option<PackageManifest> {
    let source = std::fs::read_to_string(path).ok()?;
    parse_manifest(&source)
}

pub(crate) fn parse_manifest(source: &str) -> Option<PackageManifest> {
    let root: Value = serde_yaml::from_str(source).ok()?;
    let mapping = root.as_mapping()?;
    Some(PackageManifest {
        name: mapping_string(mapping, "name")?.to_owned(),
        dependencies: mapping_keys(mapping, "dependencies")?,
        dev_dependencies: mapping_keys(mapping, "dev_dependencies")?,
    })
}

fn mapping_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping.get(Value::String(key.to_string()))?.as_str()
}

/// Dependency-section keys. `None` when the section exists but isn't a mapping —
/// a malformed manifest, which callers skip rather than treat as "no dependencies"
/// and report every import as undeclared.
fn mapping_keys(mapping: &Mapping, key: &str) -> Option<HashSet<String>> {
    match mapping.get(Value::String(key.to_string())) {
        None | Some(Value::Null) => Some(HashSet::new()),
        Some(Value::Mapping(entries)) => Some(
            entries
                .keys()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect(),
        ),
        Some(_) => None,
    }
}

pub(crate) fn imported_package(uri: &str) -> Option<&str> {
    let (package, path) = uri.strip_prefix("package:")?.split_once('/')?;
    (!package.is_empty() && !path.is_empty()).then_some(package)
}

pub(crate) fn is_library_file(file: &Path, pubspec: &Path) -> bool {
    let file = canonical_package_path(file);
    let root = pubspec.parent().unwrap_or_else(|| Path::new(""));
    file.starts_with(canonical_package_path(&root.join("lib")))
}
