//! Disallow importing another package's private `src/` libraries.
//!
//! The rule follows the analyzer's package-URI semantics: it runs only for a
//! library under the enclosing package's `lib/` directory, compares package
//! identities from `pubspec.yaml`, and checks every default and configurable
//! import URI. Export directives are not imports and are intentionally ignored.

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
        let Some(own_package) = source_package(ctx.file_path) else {
            return Vec::new();
        };
        let file = ctx.file_path.to_string_lossy().into_owned();
        let mut diags = Vec::new();
        for import in &program.imports {
            check_uri(&import.uri, &own_package, &file, &mut diags);
            for configurable in &import.configurable_uris {
                check_uri(&configurable.uri, &own_package, &file, &mut diags);
            }
        }
        diags
    }
}

fn source_package(path: &Path) -> Option<String> {
    let pubspec = crate::cross_file::enclosing_pubspec(path)?;
    let root = pubspec.parent()?;
    let canonical = crate::cross_file::canonical_or_lexical(path);
    let relative = canonical.strip_prefix(root).ok()?;
    if relative.components().next()?.as_os_str() != "lib" {
        return None;
    }
    let source = std::fs::read_to_string(pubspec).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&source).ok()?;
    yaml.as_mapping()?
        .get(serde_yaml::Value::String("name".to_string()))?
        .as_str()
        .map(str::to_string)
}

fn check_uri(uri: &StringLitNode, own: &str, file: &str, diags: &mut Vec<Diagnostic>) {
    let Some(package) = implementation_package(&uri.value) else {
        return;
    };
    if package == own {
        return;
    }
    diags.push(Diagnostic::new(
        "implementation-imports",
        Severity::Warning,
        format!("Don't import implementation files from another package ('{package}')."),
        file.to_string(),
        DiagSpan {
            start: uri.span.start,
            end: uri.span.end,
        },
    ));
}

fn implementation_package(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("package:")?;
    let path = rest.split(['?', '#']).next()?;
    let mut segments = path.split('/');
    let package = segments.next()?;
    if package.is_empty() || segments.next()? != "src" || segments.next().is_none() {
        return None;
    }
    Some(package)
}
