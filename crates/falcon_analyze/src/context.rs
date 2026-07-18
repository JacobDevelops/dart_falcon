use falcon_config::FalconConfig;

use crate::resolve::{LibraryUnit, ProjectIndex, TypeIndex};

/// Per-file analysis context passed to every rule.
///
/// The optional [`AnalyzeContext::project`] index is the seam for the
/// resolver-dependent rules: when present it carries cross-file (or, in degraded
/// single-file mode, this file's) declaration return-type facts. It defaults to
/// `None`, so every existing rule and construction site is unaffected — a rule
/// only reaches for it when it needs type facts, and must behave conservatively
/// when it is absent.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub config: &'a FalconConfig,
    /// Cross-file declaration index, when the driver has one. `None` for callers
    /// that have no project view (or have not opted into resolution).
    pub project: Option<&'a ProjectIndex>,
    /// Cross-file *type* index (kinds, supertypes, members), when the driver has
    /// one. Same opt-in and absence semantics as [`AnalyzeContext::project`].
    pub types: Option<&'a TypeIndex>,
    /// This file's library context — its part/owner siblings and the flag for an
    /// unresolved part. `None` when resolution is off or the file stands alone.
    pub library: Option<&'a LibraryUnit<'a>>,
}

impl<'a> AnalyzeContext<'a> {
    /// Construct a context without a project index (the common, non-resolving
    /// case). Equivalent to setting `project: None`.
    pub fn new(file_path: &'a std::path::Path, source: &'a str, config: &'a FalconConfig) -> Self {
        Self {
            file_path,
            source,
            config,
            project: None,
            types: None,
            library: None,
        }
    }

    /// Attach a project index, enabling resolver-dependent rules to consult it.
    pub fn with_project(mut self, project: &'a ProjectIndex) -> Self {
        self.project = Some(project);
        self
    }

    /// Attach a cross-file type index.
    pub fn with_types(mut self, types: &'a TypeIndex) -> Self {
        self.types = Some(types);
        self
    }

    /// Attach this file's library context.
    pub fn with_library(mut self, library: &'a LibraryUnit<'a>) -> Self {
        self.library = Some(library);
        self
    }

    /// Resolve a rule's options for the file under analysis: the base options
    /// patched by any `overrides` entry whose `includes` match this file (see
    /// [`FalconConfig::rule_options_for`]). Rules should prefer this over
    /// [`FalconConfig::rule_options`] so per-path override options take effect.
    pub fn rule_options(&self, group: &str, rule_name: &str) -> Option<&serde_json::Value> {
        self.config
            .rule_options_for(&self.file_path.to_string_lossy(), group, rule_name)
    }
}
