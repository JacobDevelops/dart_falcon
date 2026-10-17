//! Flags an old-style function-typed parameter written with inline parameter syntax.
//!
//! Dart lets you declare a function-typed parameter two ways. The legacy form
//! inlines the callee's own parameter list — `void f(int g(int x))` — while the
//! modern generic function type syntax writes the type up front:
//! `void f(int Function(int) g)`. The modern form is preferred because it reads
//! as an ordinary typed parameter, composes with nullability (`Function(int)?`),
//! and keeps the parameter name in its usual place. The rule detects the old form
//! by the nested parameter list the parser attaches to it; the modern form parses
//! as a plain parameter with a function type and is not flagged. Rewrite using
//! `Function` type syntax. Diagnostics cover the complete parameter declaration.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseFunctionTypeSyntaxForParameters;

impl Rule for UseFunctionTypeSyntaxForParameters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'c> {
    ctx: &'a AnalyzeContext<'c>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        if node.function_params.is_some() {
            self.diags.push(self.ctx.diagnostic(
                "use-function-type-syntax-for-parameters",
                "Use the generic function type syntax for parameters.".to_string(),
                node.span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn reports_the_complete_function_typed_parameter() {
        let source =
            "void f(int callback(String s), void other<T>(T x), {required int named(int x)}) {}";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let diagnostics = UseFunctionTypeSyntaxForParameters.analyze(&parsed.program, &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            actual,
            [
                "int callback(String s)",
                "void other<T>(T x)",
                "required int named(int x)"
            ]
        );
    }
}
