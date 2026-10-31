//! Report Flutter asset declarations without references in analyzed Dart sources.
//!
//! File and direct-directory declarations are checked, including references from
//! other analyzed packages using `packages/<name>/` or a literal `package:` argument.
//! Interpolated paths preserve declarations only when both their static prefix
//! and suffix match; literal directory paths also preserve matching assets.
//! Missing, unreadable and empty directories and resolution-variant
//! declarations are exempt. Packages using `flutter_gen` or `flutter_gen_runner`
//! are skipped because generated accessors may be excluded from analysis.
//! Manifests using YAML anchors or tags are skipped when scalar positions cannot
//! be recovered safely. Native code and unanalyzed sources cannot establish usage.

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::Path,
};

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::flutter_assets::{
    AssetEntry, asset_packages, asset_strings, dynamic_parts, literal_package, literal_value,
};

#[derive(Default)]
pub struct UnusedAssets;

impl CrossFileRule for UnusedAssets {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut packages = asset_packages(files, true);
        packages.retain(|package| !package.generated && !package.entries.is_empty());
        if packages.is_empty() {
            return Vec::new();
        }
        let references = references(files);
        let mut diagnostics = Vec::new();
        for package in packages {
            let own: HashSet<_> = package.files.iter().map(|file| &file.path).collect();
            let expanded: Vec<_> = package
                .entries
                .iter()
                .map(|entry| {
                    let variant = is_resolution_variant(&entry.path);
                    ExpandedEntry {
                        entry,
                        variant,
                        files: (!variant && entry.path.ends_with('/'))
                            .then(|| directory_files(&package.pubspec, &entry.path))
                            .flatten(),
                    }
                })
                .collect();
            let mut candidates: HashMap<_, _> = expanded
                .iter()
                .flat_map(ExpandedEntry::paths)
                .map(|path| (path, false))
                .collect();
            let directories: HashSet<_> = candidates
                .keys()
                .flat_map(|path| {
                    path.match_indices('/')
                        .map(move |(index, _)| &path[..index])
                })
                .collect();
            let prefix = package
                .name
                .as_ref()
                .map(|name| format!("packages/{name}/"));
            for (file, strings) in &references {
                let own_file = own.contains(file);
                for reference in strings {
                    let local = reference.package.as_ref().map_or(own_file, |name| {
                        package.name.as_deref() == Some(name.as_ref())
                    });
                    mark_referenced(
                        &mut candidates,
                        &directories,
                        reference,
                        local,
                        prefix.as_deref(),
                    );
                }
            }
            for expanded in &expanded {
                let Some(span) = expanded.entry.span else {
                    continue;
                };
                let mut paths = expanded.paths().peekable();
                if paths.peek().is_none() || paths.any(|path| candidates.get(path) == Some(&true)) {
                    continue;
                }
                let mut diagnostic = Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!(
                        "Asset `{}` is declared in `pubspec.yaml` but never referenced.",
                        expanded.entry.path
                    ),
                    package.pubspec.to_string_lossy().into_owned(),
                    span,
                );
                diagnostic.resolve_position(&package.source);
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }
}

struct ExpandedEntry<'a> {
    entry: &'a AssetEntry,
    files: Option<Vec<String>>,
    variant: bool,
}

impl ExpandedEntry<'_> {
    fn paths(&self) -> impl Iterator<Item = &str> {
        (!self.entry.path.ends_with('/'))
            .then_some(self.entry.path.as_str())
            .into_iter()
            .chain(self.files.iter().flatten().map(String::as_str))
            .filter(|_| !self.variant)
    }
}

enum AssetValue<'a> {
    Literal(Cow<'a, str>),
    Dynamic(String, String),
}

struct Reference<'a> {
    value: AssetValue<'a>,
    package: Option<Cow<'a, str>>,
}

fn references(files: &[ProjectFile]) -> Vec<(&std::path::PathBuf, Vec<Reference<'_>>)> {
    files
        .iter()
        .filter(|file| {
            file.path
                .extension()
                .is_some_and(|extension| extension == "dart")
        })
        .map(|file| {
            let strings = asset_strings(file)
                .filter_map(|literal| {
                    let value = if let Some(value) = literal_value(literal) {
                        AssetValue::Literal(value)
                    } else {
                        let (start, end) = dynamic_parts(literal)?;
                        AssetValue::Dynamic(start, end)
                    };
                    Some(Reference {
                        value,
                        package: literal_package(literal),
                    })
                })
                .collect();
            (&file.path, strings)
        })
        .collect()
}

fn mark_referenced(
    candidates: &mut HashMap<&str, bool>,
    directories: &HashSet<&str>,
    reference: &Reference<'_>,
    local: bool,
    prefix: Option<&str>,
) {
    match &reference.value {
        AssetValue::Literal(value) => {
            let path = prefix
                .and_then(|prefix| value.strip_prefix(prefix))
                .or_else(|| local.then_some(value.as_ref()));
            let Some(path) = path else { return };
            if let Some(used) = candidates.get_mut(path) {
                *used = true;
            }
            if !directories.contains(path.trim_end_matches('/')) {
                return;
            }
            for (candidate, used) in candidates {
                *used |= if path.ends_with('/') {
                    candidate.starts_with(path)
                } else {
                    candidate
                        .strip_prefix(path)
                        .is_some_and(|child| child.starts_with('/'))
                };
            }
        }
        AssetValue::Dynamic(start, end) => {
            for (candidate, used) in candidates {
                *used |= local && candidate.starts_with(start) && candidate.ends_with(end);
                if !*used
                    && start.starts_with("packages/")
                    && let Some(prefix) = prefix
                {
                    let qualified = prefix.bytes().chain(candidate.bytes());
                    *used = qualified.clone().take(start.len()).eq(start.bytes())
                        && qualified.rev().take(end.len()).eq(end.bytes().rev());
                }
            }
        }
    }
}

fn directory_files(pubspec: &Path, path: &str) -> Option<Vec<String>> {
    let directory = std::fs::read_dir(pubspec.parent()?.join(path)).ok()?;
    let mut paths = Vec::new();
    for child in directory {
        let child = child.ok()?;
        if std::fs::metadata(child.path()).ok()?.is_file() {
            paths.push(format!("{path}{}", child.file_name().to_str()?));
        }
    }
    Some(paths)
}

fn is_resolution_variant(path: &str) -> bool {
    path.split('/').any(|segment| {
        segment
            .strip_suffix('x')
            .and_then(|scale| scale.parse::<f64>().ok())
            .is_some_and(|scale| scale.is_finite() && scale > 0.0)
    })
}
