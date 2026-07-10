use crate::{AnalyzeContext, FileSuppressions, Rule};
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
        let diagnostics: Vec<Diagnostic> = self
            .rules
            .iter()
            .flat_map(|rule| {
                let span = debug_span!("run_rule", rule = rule.name());
                let _enter = span.enter();
                debug!(rule = rule.name(), "running rule");
                rule.analyze(program, ctx)
            })
            .collect();

        // Honor inline `// ignore:` / `// ignore_for_file:` suppressions. Parse
        // them lazily: clean files pay nothing, and files without directives
        // skip the per-diagnostic filter entirely.
        if diagnostics.is_empty() {
            return diagnostics;
        }
        let suppressions = FileSuppressions::from_source(ctx.source);
        if suppressions.is_empty() {
            return diagnostics;
        }
        diagnostics
            .into_iter()
            .filter(|diag| {
                let line = suppressions.line_for_offset(diag.span.start);
                !suppressions.is_suppressed(diag.rule, line)
            })
            .collect()
    }
}
