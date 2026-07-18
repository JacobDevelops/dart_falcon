//! Flags a `library` directive that carries a name. Ported from package:lints
//! `unnecessary_library_name`: Dart no longer requires a name on `library`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UnnecessaryLibraryName;

impl Rule for UnnecessaryLibraryName {
    fn name(&self) -> &'static str {
        "unnecessary-library-name"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        if let Some(lib) = &program.library_directive
            && let (Some(first), Some(last)) = (lib.name.first(), lib.name.last())
        {
            diags.push(Diagnostic::new(
                "unnecessary-library-name",
                Severity::Warning,
                "Library names are not necessary.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: first.span.start,
                    end: last.span.end,
                },
            ));
        }
        diags
    }
}
