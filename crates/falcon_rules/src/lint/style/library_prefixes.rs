//! Flags import prefixes that are not `lower_case_with_underscores`.
//!
//! An import prefix (`import '...' as foo;`) is a library-style name and should
//! use `lower_case_with_underscores` to match Dart's package and library
//! naming, keeping qualified references uniform. Lowercase letters, digits,
//! underscores, and `$` are all permitted; the presence of any uppercase letter
//! is what makes a prefix invalid.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct LibraryPrefixes;

impl Rule for LibraryPrefixes {
    fn name(&self) -> &'static str {
        "library-prefixes"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for import in &program.imports {
            if let Some(prefix) = &import.as_name
                && !is_valid_library_prefix(&prefix.name)
            {
                diags.push(Diagnostic::new(
                    "library-prefixes",
                    Severity::Warning,
                    MESSAGE,
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: prefix.span.start,
                        end: prefix.span.end,
                    },
                ));
            }
        }
        diags
    }
}

const MESSAGE: &str = "Use `lowercase_with_underscores` for a library prefix.";

/// `lower_case_with_underscores` forbids uppercase letters; digits, underscores
/// and `$` are all permitted. Since an identifier cannot begin with a digit,
/// the presence of any uppercase ASCII letter is the only disqualifier.
fn is_valid_library_prefix(name: &str) -> bool {
    !name.bytes().any(|b| b.is_ascii_uppercase())
}
