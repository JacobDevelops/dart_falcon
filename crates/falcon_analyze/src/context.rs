use falcon_config::FalconConfig;

use crate::resolve::ProjectIndex;

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
        }
    }

    /// Attach a project index, enabling resolver-dependent rules to consult it.
    pub fn with_project(mut self, project: &'a ProjectIndex) -> Self {
        self.project = Some(project);
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
