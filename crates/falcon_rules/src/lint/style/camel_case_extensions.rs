//! Flags named extensions whose name is not UpperCamelCase.
//!
//! Extensions are types and should follow the same naming convention as classes
//! and enums so that tooling and readers treat them consistently. Leading
//! underscores are ignored for the check; the remainder must be one or more
//! words, each starting with an uppercase letter or `$`. Unnamed extensions have
//! no identifier to check and are skipped.

use crate::lint::naming::is_upper_camel_case;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CamelCaseExtensions;

impl Rule for CamelCaseExtensions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let TopLevelDecl::Extension(ext) = decl
                && let Some(name) = &ext.name
                && !is_upper_camel_case(&name.name)
            {
                diags.push(ctx.diagnostic("camel-case-extensions", MESSAGE, name.span));
            }
        }
        diags
    }
}

const MESSAGE: &str = "Name extensions using UpperCamelCase.";
