//! Flags a `library` directive that carries an explicit name.
//!
//! Since Dart 2.19 the `library` directive no longer needs a name: `part of`
//! directives can reference their parent by URI, and library-level doc comments
//! and annotations attach to a bare `library;`. A named library is legacy syntax
//! that only adds a global identifier which can collide across packages. Remove
//! the name, keeping the `library;` directive itself if it anchors documentation
//! or annotations.

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
