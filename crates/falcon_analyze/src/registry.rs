use crate::{AnalyzeContext, FileSuppressions, Rule, RuleLookup};
use falcon_diagnostics::Diagnostic;
use tracing::{debug, debug_span};

/// Without metadata, suppression paths validate as unknown rules.
fn no_lookup(_name: &str) -> Option<(&'static str, &'static str, bool)> {
    None
}

const RULES_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Deep legal ASTs can exhaust default worker stacks; all rule-execution paths need this guard.
pub fn with_rules_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(RULES_STACK_SIZE)
            .spawn_scoped(scope, f)
            .expect("spawn rules thread")
            .join()
            .expect("rules thread panicked")
    })
}

pub struct RuleRegistry {
    rules: Vec<(&'static str, Box<dyn Rule>)>,
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

    pub fn with_lookup(lookup: RuleLookup) -> Self {
        Self {
            rules: Vec::new(),
            lookup,
        }
    }

    /// Register the canonical name supplied by the caller's metadata table.
    pub fn register(&mut self, name: &'static str, rule: Box<dyn Rule>) {
        self.rules.push((name, rule));
    }

    pub fn rules(&self) -> &[(&'static str, Box<dyn Rule>)] {
        &self.rules
    }

    pub fn run_all(
        &self,
        program: &falcon_syntax::Program,
        ctx: &AnalyzeContext,
    ) -> Vec<Diagnostic> {
        with_rules_stack(|| self.run_all_inner(program, ctx))
    }

    fn run_all_inner(
        &self,
        program: &falcon_syntax::Program,
        ctx: &AnalyzeContext,
    ) -> Vec<Diagnostic> {
        let diagnostics: Vec<Diagnostic> = self
            .rules
            .iter()
            .flat_map(|(name, rule)| {
                let span = debug_span!("run_rule", rule = name);
                let _enter = span.enter();
                debug!(rule = name, "running rule");
                rule.analyze(program, ctx)
            })
            .collect();

        // Parse even without rule diagnostics so malformed suppressions are surfaced.
        let mut diagnostics = diagnostics;
        let suppressions =
            FileSuppressions::parse(ctx.source, &ctx.file_path.to_string_lossy(), self.lookup);
        if !suppressions.is_empty() {
            diagnostics.retain(|diag| {
                let line = suppressions.line_for_offset(diag.span.start);
                !suppressions.is_suppressed(diag.rule, line)
            });
        }
        // Append after filtering so malformed suppressions cannot suppress themselves.
        diagnostics.extend(suppressions.into_diagnostics());
        diagnostics.sort_by(|left, right| {
            left.rule
                .cmp(right.rule)
                .then(left.span.start.cmp(&right.span.start))
                .then(left.span.end.cmp(&right.span.end))
                .then(left.message.cmp(&right.message))
        });
        diagnostics.dedup_by(|left, right| {
            left.rule == right.rule && left.span == right.span && left.message == right.message
        });
        diagnostics
    }
}
