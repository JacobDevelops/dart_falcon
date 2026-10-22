//! Disallow importing another package's private `src/` libraries.
//!
//! The rule follows the analyzer's package-URI semantics: it runs only for a
//! library under the enclosing package's `lib/` directory, compares package
//! identities from `pubspec.yaml`, and checks every default and configurable
//! import URI. Export directives are not imports and are intentionally ignored.

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
        let Some(own_package) = ctx.file_package().metadata()
            .filter(|_| ctx.file_package().in_directory("lib"))
            .and_then(|package| package.parsed_name()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut diags = Vec::new();
        for import in program.imports() {
            check_uri(import.uri(), own_package, ctx, &mut diags);
            for configurable in import.configurable_uris() {
                check_uri(configurable.uri(), own_package, ctx, &mut diags);
            }
        }
        RulePreparation::Diagnostics(diags)
    }
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
