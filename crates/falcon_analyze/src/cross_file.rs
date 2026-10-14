//! Cross-file rules receive the full analyzed file set.

use std::path::PathBuf;

use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct ProjectFile {
    pub path: PathBuf,
    pub source: String,
    pub program: Program,
    /// Recovery nodes can create false declarations; keep these files only for reference detection.
    pub has_parse_errors: bool,
}

pub trait CrossFileRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze_project(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic>;
}

#[derive(Default)]
pub struct CrossFileRuleRegistry {
    rules: Vec<Box<dyn CrossFileRule>>,
}

impl CrossFileRuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn CrossFileRule>) {
        self.rules.push(rule);
    }

    pub fn rules(&self) -> &[Box<dyn CrossFileRule>] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Callers apply inline suppression and per-path severity resolution.
    pub fn run_all(&self, files: &[ProjectFile], config: &FalconConfig) -> Vec<Diagnostic> {
        crate::registry::with_rules_stack(|| {
            self.rules
                .iter()
                .flat_map(|rule| rule.analyze_project(files, config))
                .collect()
        })
    }
}
