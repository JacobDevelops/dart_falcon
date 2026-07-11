use falcon_config::FalconConfig;

/// Per-file analysis context passed to every rule.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub config: &'a FalconConfig,
}

impl AnalyzeContext<'_> {
    /// Resolve a rule's options for the file under analysis: the base options
    /// patched by any `overrides` entry whose `includes` match this file (see
    /// [`FalconConfig::rule_options_for`]). Rules should prefer this over
    /// [`FalconConfig::rule_options`] so per-path override options take effect.
    pub fn rule_options(&self, group: &str, rule_name: &str) -> Option<&serde_json::Value> {
        self.config
            .rule_options_for(&self.file_path.to_string_lossy(), group, rule_name)
    }
}
