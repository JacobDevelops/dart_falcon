//! Disallow declared parameters that are never used.

/// The `avoid-unused-parameters` rule.
pub use dcl::AvoidUnusedParameters;

mod dcl {
    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;
    use falcon_syntax::visitor::{
        Visitor, walk_constructor_decl, walk_function_decl, walk_method_decl, walk_stmt,
    };

    use crate::lint::lexical_usage::{used_constructor_parameters, used_parameters};

    pub struct AvoidUnusedParameters;

    impl Rule for AvoidUnusedParameters {
        fn name(&self) -> &'static str {
            "avoid-unused-parameters"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let mut collector = Collector {
                ctx,
                diagnostics: Vec::new(),
            };
            collector.visit_program(program);
            collector.diagnostics
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
            for param in params
                .positional
                .iter()
                .chain(&params.optional_positional)
                .chain(&params.named)
            {
                let name = &param.name.name;
                if param.is_field
                    || param.is_super
                    || (!name.is_empty() && name.bytes().all(|byte| byte == b'_'))
                    || used.contains(&param.name.span.start)
                {
                    continue;
                }
                self.diagnostics.push(Diagnostic::new(
                    "avoid-unused-parameters",
                    Severity::Warning,
                    format!("Parameter '{name}' is declared but not used"),
                    self.ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: param.name.span.start,
                        end: param.name.span.end,
                    },
                ));
            }
        }
    }

    impl Visitor for Collector<'_, '_> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) {
            self.check(&node.params, node.body.as_ref(), false);
            walk_function_decl(self, node);
        }

        fn visit_method_decl(&mut self, node: &MethodDecl) {
            let exempt = node.name.name == "noSuchMethod"
                || node.annotations.iter().any(|annotation| {
                    annotation
                        .name
                        .last()
                        .is_some_and(|name| name.name == "override")
                });
            self.check(&node.params, node.body.as_ref(), exempt);
            walk_method_decl(self, node);
        }

        fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
            if node.body.is_none() && node.initializers.is_empty() {
                walk_constructor_decl(self, node);
                return;
            }
            let used =
                used_constructor_parameters(&node.params, &node.initializers, node.body.as_ref());
            self.report(&node.params, &used);
            walk_constructor_decl(self, node);
        }

        fn visit_stmt(&mut self, node: &Stmt) {
            if let Stmt::LocalFunc(function) = node {
                self.check(&function.params, Some(&function.body), false);
            }
            walk_stmt(self, node);
        }
    }
}
