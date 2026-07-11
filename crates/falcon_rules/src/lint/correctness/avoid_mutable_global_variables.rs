//! Flags mutable global variables. Ported from pyramid_lint's `avoid_mutable_global_variables`.

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
                // pyramid_lint only flags top-level variables that are neither
                // `const` nor `final` (`isConstOrFinal` skips the declaration).
                if !var_decl.is_const && !var_decl.is_final {
                    diags.push(Diagnostic::new(
                        "avoid_mutable_global_variables",
                        Severity::Warning,
                        "Avoid mutable global variables. Use const or final instead.",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: var_decl.span.start,
                            end: var_decl.span.end,
                        },
                    ));
                }
            }
        }

        diags
    }
}
