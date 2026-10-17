//! Disallow declared parameters that are never used, excluding primary constructors.

/// The `avoid-unused-parameters` rule.
pub use dcl::AvoidUnusedParameters;

mod dcl {
    use falcon_analyze::{
        AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    };
    use falcon_diagnostics::Diagnostic;
    use falcon_syntax::ast::*;

    use falcon_analyze::resolve::lexical_usage::{used_constructor_parameters, used_parameters};

    #[derive(Default)]
    pub struct AvoidUnusedParameters;

    impl Rule for AvoidUnusedParameters {
        fn prepare<'a>(
            &self,
            _program: &'a Program,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            let collector = Collector {
                ctx,
                diagnostics: Vec::new(),
            };
            RulePreparation::Structural(Box::new(collector))
        }
    }

    struct Collector<'a, 'ctx> {
        ctx: &'a AnalyzeContext<'ctx>,
        diagnostics: Vec<Diagnostic>,
    }

    impl Collector<'_, '_> {
        fn check(&mut self, params: &FormalParamList, body: Option<&FunctionBody>, exempt: bool) {
            let Some(body) = body.filter(|_| !exempt) else {
                return;
            };
            self.report(params, &used_parameters(params, body));
        }

        fn report(&mut self, params: &FormalParamList, used: &std::collections::HashSet<usize>) {
            for param in params.params.iter() {
                let name = &param.name.name;
                if param.initializing.is_some()
                    || (!name.is_empty() && name.bytes().all(|byte| byte == b'_'))
                    || used.contains(&param.name.span.start)
                {
                    continue;
                }
                self.diagnostics.push(self.ctx.diagnostic(
                    "avoid-unused-parameters",
                    format!("Parameter '{name}' is declared but not used"),
                    param.name.span,
                ));
            }
        }
    }

    falcon_analyze::structural_visitor! {
    impl StructuralRuleVisitor for Collector<'_, '_> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
            if let Some(params) = node.params() {
                self.check(params, node.body.as_ref(), false);
            }
            StructuralTraversal::Continue
        }

        fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
            let exempt = node.name.name == "noSuchMethod"
                || node.annotations.iter().any(|annotation| {
                    annotation
                        .name
                        .last()
                        .is_some_and(|name| name.name == "override")
                });
            self.check(&node.params, node.body.as_ref(), exempt);
            StructuralTraversal::Continue
        }

        fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
            if node.is_primary || (node.body.is_none() && node.initializers.is_empty()) {
                return StructuralTraversal::Continue;
            }
            let used = used_constructor_parameters(node.params, node.initializers, node.body);
            self.report(node.params, &used);
            StructuralTraversal::Continue
        }

        fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
            if let Stmt::LocalFunc(function) = node {
                self.check(&function.params, Some(&function.body), false);
            }
            StructuralTraversal::Continue
        }

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            self.diagnostics
        }
    }
    }
}
