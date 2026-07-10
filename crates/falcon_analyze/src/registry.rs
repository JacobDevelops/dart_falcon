use crate::{AnalyzeContext, FileSuppressions, Rule, RuleLookup};
use falcon_diagnostics::Diagnostic;
use tracing::{debug, debug_span};

/// Default rule lookup for a registry built without one: knows no rules, so
/// every suppression path validates as "unknown rule". Real callers install a
/// lookup backed by `falcon_rules` metadata via [`RuleRegistry::with_lookup`].
fn no_lookup(_name: &str) -> Option<(&'static str, bool)> {
    None
}

/// Registry of enabled lint rules.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
    lookup: RuleLookup,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            lookup: no_lookup,
        }
    }

    /// Build a registry whose suppression paths are validated against `lookup`
    /// (supplied from `falcon_rules` metadata at the call site).
    pub fn with_lookup(lookup: RuleLookup) -> Self {
        Self {
            rules: Vec::new(),
            lookup,
        }
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

        // Honor inline `// falcon-ignore` / `// falcon-ignore-all` suppressions.
        // Parse always (even with no rule diagnostics) so malformed comments are
        // surfaced; a fast path inside `parse` keeps clean files cheap.
        let mut diagnostics = diagnostics;
        let suppressions =
            FileSuppressions::parse(ctx.source, &ctx.file_path.to_string_lossy(), self.lookup);
        if !suppressions.is_empty() {
            diagnostics.retain(|diag| {
                let line = suppressions.line_for_offset(diag.span.start);
                !suppressions.is_suppressed(diag.rule, line)
            });
        }
        // Malformed-suppression diagnostics are appended after filtering so they
        // cannot suppress themselves.
        diagnostics.extend(suppressions.into_diagnostics());
        diagnostics
    }
}
