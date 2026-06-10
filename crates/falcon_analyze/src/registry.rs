use crate::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use tracing::{debug, debug_span};

/// Registry of enabled lint rules.
#[derive(Default)]
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Get an immutable reference to all registered rules.
    pub fn rules(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Run all registered rules on a program and return combined diagnostics.
    pub fn run_all(
        &self,
        program: &falcon_syntax::Program,
        ctx: &AnalyzeContext,
    ) -> Vec<Diagnostic> {
        self.rules
            .iter()
            .flat_map(|rule| {
                let span = debug_span!("run_rule", rule = rule.name());
                let _enter = span.enter();
                debug!(rule = rule.name(), "running rule");
                rule.analyze(program, ctx)
            })
            .collect()
    }
}
