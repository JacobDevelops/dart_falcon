use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

#[derive(Default)]
pub struct ScaffoldRule;

impl CrossFileRule for ScaffoldRule {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let _ = (files, config, indexes);
        Vec::new()
    }
}
