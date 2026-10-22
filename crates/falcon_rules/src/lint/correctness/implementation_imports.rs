//! Disallow importing another package's private `src/` libraries.
//!
//! The rule follows the analyzer's package-URI semantics: it runs only for a
//! library under the enclosing package's `lib/` directory, compares package
//! identities from `pubspec.yaml`, and checks every default and configurable
//! import URI. Export directives are not imports and are intentionally ignored.

use std::path::Path;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ImplementationImports;

impl Rule for ImplementationImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(own_package) = source_package(ctx.file_path()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut diags = Vec::new();
        for import in program.imports() {
            check_uri(import.uri(), &own_package, ctx, &mut diags);
            for configurable in import.configurable_uris() {
                check_uri(configurable.uri(), &own_package, ctx, &mut diags);
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn source_package(path: &Path) -> Option<String> {
    let pubspec = crate::cross_file::enclosing_pubspec(path)?;
    let root = pubspec.parent()?;
    let canonical = falcon_analyze::canonical_or_lexical(path);
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

fn check_uri(uri: StringLitNode<'_>, own: &str, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let value = uri.value();
    let Some(package) = implementation_package(&value) else {
        return;
    };
    if package == own {
        return;
    }
    diags.push(ctx.diagnostic(format!("Don't import implementation files from another package (`{package}`)."), uri.span()));
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
