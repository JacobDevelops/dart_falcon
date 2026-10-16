//! Flags import prefixes that are not `lower_case_with_underscores`.
//!
//! An import prefix (`import '...' as foo;`) is a library-style name and should
//! use `lower_case_with_underscores` to match Dart's package and library
//! naming. Prefixes may have one leading `$` and leading underscores, then must
//! start with a lowercase letter and contain only lowercase letters, digits,
//! and underscores.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::language_version::Feature;

#[derive(Default)]
pub struct LibraryPrefixes;

impl Rule for LibraryPrefixes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for import in &program.imports {
            if let Some(prefix) = &import.as_name
                && !(prefix.name == "_"
                    && ctx.language_version().supports(Feature::WildcardVariables))
                && !is_valid_library_prefix(&prefix.name)
            {
                diags.push(ctx.diagnostic("library-prefixes", MESSAGE, prefix.span));
            }
        }
        diags
    }
}

const MESSAGE: &str = "Use `lowercase_with_underscores` for a library prefix.";

fn is_valid_library_prefix(name: &str) -> bool {
    let rest = name
        .strip_prefix('$')
        .unwrap_or(name)
        .trim_start_matches('_');
    rest.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
