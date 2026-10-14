//! Disallow mutable top-level variables.
//!
//! Flags a top-level variable that is neither `const` nor `final`. A mutable
//! global can be reassigned from anywhere in the program, so its value at any
//! moment depends on execution order that no local reading of the code reveals —
//! a recurring source of subtle bugs and flaky, order-dependent tests. Declare
//! it `const` or `final`; when a value genuinely must change over time, hold it
//! in an object with a clear owner rather than a free-floating global.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

pub struct AvoidMutableGlobalVariables;

impl Rule for AvoidMutableGlobalVariables {
    fn name(&self) -> &'static str {
        "avoid-mutable-global-variables"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            if let TopLevelDecl::Variable(var_decl) = decl {
                // pyramid_lint only flags top-level variables that are neither
                // `const` nor `final` (`isConstOrFinal` skips the declaration).
                if var_decl.keyword == falcon_syntax::ast::VarKeyword::Var {
                    diags.push(ctx.diagnostic(
                        "avoid-mutable-global-variables",
                        "Avoid mutable global variables. Use const or final instead.",
                        var_decl.span,
                    ));
                }
            }
        }

        diags
    }
}
