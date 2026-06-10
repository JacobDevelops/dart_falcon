use falcon_config::FalconConfig;

/// Per-file analysis context passed to every rule.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub config: &'a FalconConfig,
}
