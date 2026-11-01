use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct PackageManifest {
    pub(crate) name: String,
    pub(crate) dependencies: HashSet<String>,
    pub(crate) dev_dependencies: HashSet<String>,
}

pub(crate) fn manifest_facts(package: &falcon_analyze::PackageMetadata) -> Option<PackageManifest> {
    let falcon_analyze::ManifestSnapshot::Valid { yaml, .. } = package.snapshot() else {
        return None;
    };
    from_yaml(yaml)
}

pub(crate) struct ManifestSections<'a> {
    name: &'a str,
    dependencies: DependencySection<'a>,
    dev_dependencies: DependencySection<'a>,
}

#[derive(Clone, Copy)]
enum DependencySection<'a> {
    Empty,
    Mapping(&'a Mapping),
}

pub(crate) fn manifest_sections(root: &Value) -> Option<ManifestSections<'_>> {
    let mapping = root.as_mapping()?;
    Some(ManifestSections {
        name: mapping.get("name")?.as_str()?,
        dependencies: dependency_section(mapping, "dependencies")?,
        dev_dependencies: dependency_section(mapping, "dev_dependencies")?,
    })
}

fn from_yaml(root: &Value) -> Option<PackageManifest> {
    let sections = manifest_sections(root)?;
    Some(PackageManifest {
        name: sections.name.to_owned(),
        dependencies: dependency_names(sections.dependencies),
        dev_dependencies: dependency_names(sections.dev_dependencies),
    })
}

fn dependency_section<'a>(mapping: &'a Mapping, key: &str) -> Option<DependencySection<'a>> {
    match mapping.get(key) {
        None | Some(Value::Null) => Some(DependencySection::Empty),
        Some(Value::Mapping(entries)) => Some(DependencySection::Mapping(entries)),
        Some(_) => None,
    }
}

fn dependency_names(section: DependencySection<'_>) -> HashSet<String> {
    let DependencySection::Mapping(entries) = section else {
        return HashSet::new();
    };
    entries
        .keys()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn imported_package(uri: &str) -> Option<&str> {
    let (package, path) = uri.strip_prefix("package:")?.split_once('/')?;
    (!package.is_empty() && !path.is_empty()).then_some(package)
}

pub(crate) fn is_library_file(
    environment: &falcon_analyze::AnalysisEnvironment,
    file: &Path,
    pubspec: &Path,
) -> bool {
    let file = environment.canonical_path(file);
    let root = pubspec.parent().unwrap_or_else(|| Path::new(""));
    file.starts_with(environment.canonical_path(&root.join("lib")))
}
