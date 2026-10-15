//! Identical rule reports are normalized at the common CLI/LSP registry boundary.

#[path = "support/inference.rs"]
mod inference;

use std::path::Path;

use falcon_analyze::{AnalyzeContext, Rule, RuleRegistry};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_rules::all_rules;
use falcon_rules::lint::performance::noop_primitive_operations::NoopPrimitiveOperations;
use falcon_syntax::Program;

struct RegisteredRules(RuleRegistry);

impl Rule for RegisteredRules {
    fn analyze(&self, program: &Program, context: &AnalyzeContext) -> Vec<Diagnostic> {
        self.0.run_all(program, context)
    }
}

#[test]
fn overlapping_primitive_checks_emit_one_report_per_span() {
    let mut registry = RuleRegistry::new();
    registry.register(
        "noop-primitive-operations",
        Box::new(NoopPrimitiveOperations),
    );
    let source = "void f(String s) { print(s.toString()); print('${s.toString()}'); }";
    let diagnostics = inference::analyze(
        &RegisteredRules(registry),
        Path::new("dedup.dart"),
        source,
        &FalconConfig::default(),
    );
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_ne!(diagnostics[0].span, diagnostics[1].span);
    for diagnostic in diagnostics {
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            "toString"
        );
    }
}
