//! Ban decoded export URIs, including all conditional alternatives.

use crate::lint::style::conditional_uri_does_not_exist::uri_value;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use super::avoid_banned_names::{entries, report};

#[derive(Default)]
pub struct AvoidBannedExports;

impl Rule for AvoidBannedExports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = entries(ctx, "avoid-banned-exports");
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
                        &entries,
                        "avoid-banned-exports",
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
