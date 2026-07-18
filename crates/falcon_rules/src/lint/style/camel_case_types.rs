//! Flags type names that are not UpperCamelCase.
//!
//! Classes, mixins, enums, extension types, and type aliases are all types and
//! read most predictably when named in UpperCamelCase, matching the rest of the
//! Dart ecosystem. Leading underscores are ignored, then each word must begin
//! with an uppercase letter or `$`; a name that is entirely underscores is
//! treated as a wildcard and never flagged.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct CamelCaseTypes;

impl Rule for CamelCaseTypes {
    fn name(&self) -> &'static str {
        "camel-case-types"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => check(&c.name, &mut diags, ctx),
                TopLevelDecl::MixinClass(c) => check(&c.name, &mut diags, ctx),
                TopLevelDecl::Mixin(m) => check(&m.name, &mut diags, ctx),
                TopLevelDecl::Enum(e) => check(&e.name, &mut diags, ctx),
                TopLevelDecl::ExtensionType(x) => check(&x.name, &mut diags, ctx),
                TopLevelDecl::TypeAlias(t) => check(&t.name, &mut diags, ctx),
                _ => {}
            }
        }
        diags
    }
}

const MESSAGE: &str = "Name types using UpperCamelCase.";

/// UpperCamelCase per the analyzer's `isCamelCase`: leading underscores are
/// ignored, then the remainder must be one or more words each starting with an
/// uppercase letter or `$` (`([A-Z$][a-z0-9$]*)+`). A name that is entirely
/// underscores is treated as valid (never flagged).
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

fn check(name: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !is_upper_camel_case(&name.name) {
        diags.push(Diagnostic::new(
            "camel-case-types",
            Severity::Warning,
            MESSAGE,
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: name.span.start,
                end: name.span.end,
            },
        ));
    }
}
