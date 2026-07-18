//! Disallow relative imports that reach into a `lib/` directory.
//!
//! Flags a relative `import` whose path contains a `lib` segment, such as
//! `import '../lib/foo.dart'` or `import 'lib/foo.dart'`; `package:` and `dart:`
//! imports are exempt. Dart treats a file reached by a relative path and the
//! same file reached by its `package:` URI as two separate libraries, so
//! importing into `lib/` relatively can create duplicate type identities,
//! baffling "X is not a Y" errors, and `is` checks that fail against seemingly
//! identical types. Import files under `lib/` through their `package:` URI
//! instead.

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
