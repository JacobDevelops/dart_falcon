//! Disallow importing web-only `dart:` libraries from Flutter code.
//!
//! Flags any `import` of `dart:html`, `dart:js`, `dart:js_util`, or a
//! `dart:js_interop*` library. These libraries exist only on the web platform,
//! so code that depends on them fails to compile or run on Android, iOS,
//! desktop, and embedded targets. A Flutter package that reaches for them
//! silently forfeits portability across every platform Flutter otherwise
//! supports. Use platform-neutral APIs, or isolate web-specific code behind a
//! conditional import so non-web builds receive a compatible implementation.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidWebLibrariesInFlutter;

impl Rule for AvoidWebLibrariesInFlutter {
    fn name(&self) -> &'static str {
        "avoid-web-libraries-in-flutter"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for import in &program.imports {
            if is_web_library(&import.uri.value) {
                diags.push(Diagnostic::new(
                    "avoid-web-libraries-in-flutter",
                    Severity::Warning,
                    "Avoid using web-only libraries in Flutter code; they are not portable across platforms.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: import.span.start,
                        end: import.span.end,
                    },
                ));
            }
        }
        diags
    }
}

/// Web-only core libraries: `dart:html`, `dart:js`, `dart:js_util`, and any
/// `dart:js_interop*` variant (`dart:js_interop`, `dart:js_interop_unsafe`).
fn is_web_library(uri: &str) -> bool {
    matches!(uri, "dart:html" | "dart:js" | "dart:js_util") || uri.starts_with("dart:js_interop")
}
