//! Flags named extensions whose name is not UpperCamelCase.
//!
//! Extensions are types and should follow the same naming convention as classes
//! and enums so that tooling and readers treat them consistently. Leading
//! underscores are ignored for the check; the remainder must be one or more
//! words, each starting with an uppercase letter or `$`. Unnamed extensions have
//! no identifier to check and are skipped.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

pub struct CamelCaseExtensions;

impl Rule for CamelCaseExtensions {
    fn name(&self) -> &'static str {
        "camel-case-extensions"
    }

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

/// UpperCamelCase per the analyzer's `isCamelCase`: leading underscores are
/// ignored, then the remainder must be one or more words each starting with an
/// uppercase letter or `$` (`([A-Z$][a-z0-9$]*)+`).
fn is_upper_camel_case(name: &str) -> bool {
    let rest = name.trim_start_matches('_');
    let Some(first) = rest.chars().next() else {
        return true;
    };
    if !(first.is_ascii_uppercase() || first == '$') {
        return false;
    }
    rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}
