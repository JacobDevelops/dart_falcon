//! Warns when package dependencies cross clean-architecture layer or feature boundaries.
//!
//! Imports, conditional alternatives, exports, and parts within a known package's
//! `lib/` are classified by the innermost recognized directory, ignoring feature
//! root/name components and treating use cases nested under domain as domain:
//! presentation (`presentation`, `ui`, `view`, `views`, `widgets`, `screens`,
//! `pages`), application (`application`, `use_cases`, `usecases`), domain
//! (`domain`), or data (`data`, `infrastructure`). Presentation may depend on
//! application or domain; application
//! and data may depend on domain. Same-layer dependencies are allowed.
//!
//! Between different features directly under `lib/features/`, `lib/feature/`, or
//! `lib/modules/`, dependencies must target a directory tree containing `domain`
//! or a file directly in the feature root. Layer direction still applies; a
//! forbidden layer direction takes precedence over the feature diagnostic.
//! Unclassified endpoints, files outside `lib/`, unknown packages, and external
//! packages are ignored. Classification is path-based, not semantic; public
//! barrels are identified by their location alone.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use falcon_analyze::{AnalysisEnvironment, CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};

use super::{ResolvedRef, canonical_package_path, program_reference_uris, resolve_directive_uri};

#[derive(Default)]
pub struct ArchitectureLayerBoundaries;

impl CrossFileRule for ArchitectureLayerBoundaries {
    fn prepare_project(
        &self,
        environment: &AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut target_packages: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
        for file in files {
            let file_package = environment.file_package(&file.path);
            let Some(package) = file_package.identity() else {
                continue;
            };
            let source = canonical_package_path(&file.path);
            let Ok(source_relative) = source.strip_prefix(&package.lib_root) else {
                continue;
            };
            let Some(source_layer) = Layer::of(source_relative) else {
                continue;
            };
            for uri in program_reference_uris(file.program()) {
                let Some(ResolvedRef::Path(target)) = resolve_directive_uri(
                    &source,
                    &uri.value(),
                    Some(package),
                    std::slice::from_ref(package),
                ) else {
                    continue;
                };
                let Ok(target_relative) = target.strip_prefix(&package.lib_root) else {
                    continue;
                };
                let Some(target_layer) = Layer::of(target_relative) else {
                    continue;
                };
                let Some(directory) = target.parent() else {
                    continue;
                };
                if !target_packages.contains_key(directory) {
                    let identity = environment
                        .file_package(&target)
                        .identity()
                        .map(|identity| identity.lib_root.clone());
                    target_packages.insert(directory.to_path_buf(), identity);
                }
                if target_packages.get(directory).and_then(Option::as_deref)
                    != Some(package.lib_root.as_path())
                {
                    continue;
                }
                let message = if !source_layer.allows(target_layer) {
                    format!(
                        "The `{}` layer must not import from the `{}` layer.",
                        source_layer.name(),
                        target_layer.name()
                    )
                } else if !layer_path(target_relative).parent().is_some_and(|parent| {
                    parent
                        .components()
                        .any(|component| component.as_os_str() == "domain")
                }) && feature(source_relative)
                    .zip(feature(target_relative))
                    .is_some_and(|(source, target)| source != target)
                {
                    "Import another feature only through its `domain` layer or public barrel."
                        .to_owned()
                } else {
                    continue;
                };
                diagnostics.push(Diagnostic::new(
                    "",
                    Severity::Warning,
                    message,
                    file.path.to_string_lossy().as_ref(),
                    uri.span(),
                ));
            }
        }
        diagnostics
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Presentation,
    Application,
    Domain,
    Data,
}

impl Layer {
    fn of(relative: &Path) -> Option<Self> {
        let mut layer = None;
        let mut under_domain = false;
        for component in layer_path(relative).parent()?.components() {
            let matched = match component.as_os_str().to_str() {
                Some(
                    "presentation" | "ui" | "view" | "views" | "widgets" | "screens" | "pages",
                ) => Some(Self::Presentation),
                Some("use_cases" | "usecases") if under_domain => Some(Self::Domain),
                Some("application" | "use_cases" | "usecases") => Some(Self::Application),
                Some("domain") => {
                    under_domain = true;
                    Some(Self::Domain)
                }
                Some("data" | "infrastructure") => Some(Self::Data),
                _ => None,
            };
            if matched.is_some() {
                layer = matched;
            }
        }
        layer
    }

    fn allows(self, target: Self) -> bool {
        self == target
            || target == Self::Domain
            || (self == Self::Presentation && target == Self::Application)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Presentation => "presentation",
            Self::Application => "application",
            Self::Domain => "domain",
            Self::Data => "data",
        }
    }
}

fn feature(relative: &Path) -> Option<&OsStr> {
    let mut components = relative.components();
    match components.next()?.as_os_str().to_str()? {
        "features" | "feature" | "modules" => Some(components.next()?.as_os_str()),
        _ => None,
    }
}

fn layer_path(relative: &Path) -> &Path {
    if feature(relative).is_none() {
        return relative;
    }
    let mut components = relative.components();
    components.next();
    components.next();
    components.as_path()
}
