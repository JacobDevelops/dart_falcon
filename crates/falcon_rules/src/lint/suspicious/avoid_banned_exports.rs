//! Ban decoded export URIs, including all conditional alternatives.

use crate::lint::support::directive_strings::uri_value;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use crate::lint::support::banned_entries::{Entry, report};

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Banned export-URI globs and the diagnostic message to report on a match.
    rules: Vec<Entry>,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidBannedExports;

impl Rule for AvoidBannedExports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = &crate::lint::options::get::<Options>(ctx, Self::NAME).rules;
        let mut diagnostics = Vec::new();
        if !entries.is_empty() {
            for export in program.exports() {
                for uri in std::iter::once(export.uri()).chain(
                    export
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                ) {
                    report(
                        ctx,
                        entries,
                        &uri_value(uri),
                        uri.span(),
                        &mut diagnostics,
                    );
                }
            }
        }
        RulePreparation::Diagnostics(diagnostics)
    }
}
