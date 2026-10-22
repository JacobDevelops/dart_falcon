use crate::lint::options::{Glob, nonblank_message, nonempty_patterns};
use falcon_analyze::AnalyzeContext;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Span;
use glob::MatchOptions;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Entry {
    /// Non-empty glob patterns (JSON key `deny`) that trigger this entry.
    #[serde(rename = "deny", deserialize_with = "nonempty_patterns")]
    #[schemars(length(min = 1))]
    patterns: Vec<Glob>,
    /// Diagnostic text emitted when any pattern in this entry matches.
    #[serde(deserialize_with = "nonblank_message")]
    message: String,
}
pub(crate) fn report(
    ctx: &AnalyzeContext<'_>,
    entries: &[Entry],
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const MATCH: MatchOptions = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    for entry in entries {
        if entry
            .patterns
            .iter()
            .any(|pattern| pattern.matches_with(value, MATCH))
        {
            diagnostics.push(ctx.diagnostic(&entry.message, span));
        }
    }
}
