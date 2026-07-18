//! Flags a `part of` directive that names its library instead of using a string URI.
//!
//! A `part of my.library.name;` directive identifies the enclosing library by its
//! declared name, which forces the parent to carry a `library` name and couples the
//! part to it indirectly. The URI form, `part of 'parent.dart';`, points straight at
//! the containing file, is more robust, and is the form modern Dart tooling prefers.
//! The rule reports only the named form (a non-empty dotted name with no string URI).

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UseStringInPartOfDirectives;

impl Rule for UseStringInPartOfDirectives {
    fn name(&self) -> &'static str {
        "use-string-in-part-of-directives"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        if let Some(part_of) = &program.part_of_directive
            && part_of.uri.is_none()
            && !part_of.name.is_empty()
        {
            diags.push(Diagnostic::new(
                "use-string-in-part-of-directives",
                Severity::Warning,
                "Use a string to refer to the containing library in a part-of directive.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: part_of.span.start,
                    end: part_of.span.end,
                },
            ));
        }
        diags
    }
}
