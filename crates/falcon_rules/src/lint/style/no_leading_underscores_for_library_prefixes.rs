//! Flags import prefixes that begin with an underscore.
//!
//! In Dart a leading underscore signals library privacy, but an import prefix
//! is only a local alias — it is never exported and cannot be made private, so
//! the underscore is meaningless noise that only muddies the privacy
//! convention. Rename the prefix without the leading underscore. A wildcard
//! prefix composed solely of underscores (e.g. `_`) is exempt, since it is an
//! intentional throwaway name rather than a misapplied privacy marker.

use crate::lint::naming::has_leading_underscore;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoLeadingUnderscoresForLibraryPrefixes;

impl Rule for NoLeadingUnderscoresForLibraryPrefixes {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for import in program.imports() {
            if let Some(prefix) = import.as_name()
                && has_leading_underscore(prefix.name())
            {
                diags.push(ctx.diagnostic(
                    "no-leading-underscores-for-library-prefixes",
                    MESSAGE,
                    prefix.span(),
                ));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

const MESSAGE: &str = "Avoid leading underscores for library prefixes.";
