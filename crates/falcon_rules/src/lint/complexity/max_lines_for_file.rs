//! Flags a file longer than the configured line limit.
//!
//! Very long files are hard to navigate and usually mix unrelated
//! responsibilities; splitting them improves readability and review. The rule
//! counts the total lines in the file and reports once, at the top, when the
//! count exceeds the threshold. The diagnostic message states the configured
//! limit.
//!
//! ## Options
//!
//! `max-lines` (integer, default: 200) — flag files with more than this many
//! lines.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of source lines allowed in a file.
    #[serde(deserialize_with = "falcon_config::integer")]
    max_lines: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_lines: 200 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct MaxLinesForFile;

fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, MaxLinesForFile::NAME).max_lines
}

impl Rule for MaxLinesForFile {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();

        let line_count = ctx.source().lines().count();
        let threshold = max_lines_option(ctx);

        if line_count > threshold {
            // Report on line 1 (byte offset 0). Message states the actual threshold.
            diags.push(ctx.diagnostic(format!("File exceeds the maximum number of lines ({threshold})."), DiagSpan { start: 0, end: 0 }));
        }

        RulePreparation::Diagnostics(diags)
    }
}
