//! Flags `import 'package:foo/src/...'` that reaches into another package's
//! private `src/` tree. Ported from package:lints `implementation_imports`.

use std::path::Path;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ImplementationImports;

impl Rule for ImplementationImports {
    fn name(&self) -> &'static str {
        "implementation-imports"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // ponytail: own package is inferred purely from the file path
        // (`<pkg>/lib/...`), never from pubspec. Files with no `lib` segment on
        // their path (e.g. corpus fixtures) get `None`, so every
        // `package:X/src/...` import is treated as another package's.
        let own = own_package(ctx.file_path);

        let mut diags = Vec::new();
        for import in &program.imports {
            if let Some(pkg) = other_package_src_import(&import.uri.value, own.as_deref()) {
                diags.push(Diagnostic::new(
                    "implementation-imports",
                    Severity::Warning,
                    format!("Don't import implementation files from another package ('{pkg}')."),
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

/// The package name for a file living at `<pkg>/lib/...`, if the path has a
/// `lib` segment.
fn own_package(path: &Path) -> Option<String> {
    let comps: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    comps
        .iter()
        .position(|c| *c == "lib")
        .filter(|&i| i > 0)
        .map(|i| comps[i - 1].to_string())
}

/// If `uri` is `package:X/src/...` for a package `X` other than `own`, returns
/// `X`. Otherwise `None` (own package, non-`src`, or not a `package:` URI).
fn other_package_src_import<'a>(uri: &'a str, own: Option<&str>) -> Option<&'a str> {
    let rest = uri.strip_prefix("package:")?;
    let (pkg, path) = rest.split_once('/')?;
    if !path.starts_with("src/") {
        return None;
    }
    if Some(pkg) == own {
        return None;
    }
    Some(pkg)
}
