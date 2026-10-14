//! Require the targets of conditional import and export URIs to exist.
//!
//! Adopted from the official `conditional_uri_does_not_exist` lint. Every
//! configurable URI (`if (dart.library.io) 'io_impl.dart'`) on an `import` or
//! `export` is resolved; when it names a file that is not on disk the URI is
//! reported. Relative URIs resolve against the importing file, and `package:`
//! URIs naming the enclosing package (nearest `pubspec.yaml`) resolve into its
//! `lib/` directory. falcon has no package configuration or SDK library list,
//! so `dart:` URIs, other packages' URIs, and URIs with percent escapes, query,
//! or fragment parts are never reported. The default (unconditional) URI is
//! the analyzer's own `uri_does_not_exist` diagnostic and is not checked.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::cross_file::{ResolvedRef, enclosing_package, resolve_directive_uri};
use crate::lint::correctness::always_use_package_imports::uri_scheme;
use crate::lint::string_lit::decode_escapes;

#[derive(Default)]
pub struct ConditionalUriDoesNotExist;

const NAME: &str = "conditional-uri-does-not-exist";

impl Rule for ConditionalUriDoesNotExist {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut configurations = program
            .imports
            .iter()
            .flat_map(|import| &import.configurable_uris)
            .chain(
                program
                    .exports
                    .iter()
                    .flat_map(|export| &export.configurable_uris),
            )
            .peekable();
        if configurations.peek().is_none() {
            return Vec::new();
        }
        let package = enclosing_package(ctx.file_path);
        configurations
            .filter_map(|configuration| {
                let uri = &configuration.uri;
                let value = if uri.has_raw_segment {
                    uri.value.as_str().into()
                } else {
                    decode_escapes(&uri.value)
                };
                if !matches!(uri_scheme(&value), Some("" | "package"))
                    || value.is_empty()
                    || value.contains(['%', '?', '#'])
                {
                    return None;
                }
                let Some(ResolvedRef::Path(target)) =
                    resolve_directive_uri(ctx.file_path, &value, package.as_ref())
                else {
                    return None;
                };
                (!target.is_file()).then(|| {
                    ctx.diagnostic(
                        NAME,
                        format!("The target of the conditional URI '{value}' doesn't exist."),
                        uri.span,
                    )
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ConditionalUriDoesNotExist;

    #[test]
    fn project_fixture_matches_expectations() {
        crate::cross_file::assert_project_fixture(
            "conditional-uri-does-not-exist",
            &ConditionalUriDoesNotExist,
        );
    }
}
