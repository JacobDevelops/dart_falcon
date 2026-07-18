//! Flags relative imports that reach into a `lib/` directory. Ported from
//! package:lints `avoid_relative_lib_imports`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidRelativeLibImports;

impl Rule for AvoidRelativeLibImports {
    fn name(&self) -> &'static str {
        "avoid-relative-lib-imports"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for import in &program.imports {
            if reaches_into_lib(&import.uri.value) {
                diags.push(Diagnostic::new(
                    "avoid-relative-lib-imports",
                    Severity::Warning,
                    "Avoid relative imports for files in 'lib'.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: import.uri.span.start,
                        end: import.uri.span.end,
                    },
                ));
            }
        }
        diags
    }
}

/// A relative URI (not `package:`/`dart:`) whose path contains a `lib` segment.
fn reaches_into_lib(uri: &str) -> bool {
    if uri.starts_with("package:") || uri.starts_with("dart:") {
        return false;
    }
    uri.split('/').any(|segment| segment == "lib")
}
