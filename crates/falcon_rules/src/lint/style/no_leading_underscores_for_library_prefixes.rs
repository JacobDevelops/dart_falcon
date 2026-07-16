//! Flags import prefixes that begin with an underscore. Ported from
//! package:lints `no_leading_underscores_for_library_prefixes`. A wildcard
//! prefix consisting solely of underscores (e.g. `_`) is exempt.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoLeadingUnderscoresForLibraryPrefixes;

impl Rule for NoLeadingUnderscoresForLibraryPrefixes {
    fn name(&self) -> &'static str {
        "no-leading-underscores-for-library-prefixes"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for import in &program.imports {
            if let Some(prefix) = &import.as_name
                && has_leading_underscore(&prefix.name)
            {
                diags.push(Diagnostic::new(
                    "no-leading-underscores-for-library-prefixes",
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

const MESSAGE: &str = "Avoid leading underscores for library prefixes.";

/// A name has a disallowed leading underscore when it starts with `_` and is not
/// composed solely of underscores (an all-underscore name is a wildcard).
fn has_leading_underscore(name: &str) -> bool {
    name.starts_with('_') && !name.bytes().all(|b| b == b'_')
}
