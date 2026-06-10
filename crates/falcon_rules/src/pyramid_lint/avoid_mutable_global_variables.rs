use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidMutableGlobalVariables;

impl Rule for AvoidMutableGlobalVariables {
    fn name(&self) -> &'static str {
        "avoid_mutable_global_variables"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            if let TopLevelDecl::Variable(var_decl) = decl {
                // Check if variable is mutable (not const)
                // Pyramid_lint is stricter: final and non-final mutable globals are flagged
                // unless they are const
                if !var_decl.is_const {
                    // Flag each declarator in a mutable global variable
                    for declarator in &var_decl.declarators {
                        diags.push(Diagnostic::new(
                            "avoid_mutable_global_variables",
                            Severity::Warning,
                            "Avoid mutable global variables. Use const instead.",
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan {
                                start: declarator.span.start,
                                end: declarator.span.end,
                            },
                        ));
                    }
                }
            }
        }

        diags
    }
}
