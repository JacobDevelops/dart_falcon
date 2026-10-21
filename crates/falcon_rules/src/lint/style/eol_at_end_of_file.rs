//! Flags a non-empty file that does not end with exactly one line break.
//!
//! A missing final newline breaks tools that process files line by line, and
//! extra blank lines at the end are noise. Both are reported at the end of the
//! last non-whitespace character:
//!
//! ```text
//! void f() {}⏎        // fine
//! void f() {}         // flagged: no newline at the end
//! void f() {}⏎⏎       // flagged: more than one newline at the end
//! ```
//!
//! Adopted from the official `eol_at_end_of_file` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct EolAtEndOfFile;

impl Rule for EolAtEndOfFile {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let source = ctx.source();
        if source.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let message = if ["\n\n", "\r\r", "\r\n\r\n"]
            .iter()
            .any(|ending| source.ends_with(ending))
        {
            "Multiple newlines at the end of the file."
        } else if !source.ends_with(['\n', '\r']) {
            "Missing a newline at the end of the file."
        } else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let start = source.trim_end().len();
        let end = source[start..]
            .chars()
            .next()
            .map_or(start + 1, |character| start + character.len_utf8());
        RulePreparation::Diagnostics(vec![ctx.diagnostic(
            message,
            DiagSpan { start, end },
        )])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn reports_one_character_even_past_eof() {
        for (source, suffix) in [
            ("void f() {}", None),
            ("void f() {}  ", Some(" ")),
            ("void f() {}\n\n", Some("\n")),
            ("void f() {}\r\n\r\n", Some("\r")),
            ("void f() {}\u{3000}", Some("\u{3000}")),
        ] {
            let parsed = parse(source);
            let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: parsed.tree,
            });
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
            let diagnostics = EolAtEndOfFile.analyze(EolAtEndOfFile::NAME, parsed.tree.program(source), &ctx);
            assert_eq!(diagnostics.len(), 1);
            let span = &diagnostics[0].span;
            assert_eq!(&source[..span.start], "void f() {}");
            assert_eq!(span.end - span.start, suffix.map_or(1, str::len));
            assert_eq!(source.get(span.start..span.end), suffix);
        }
    }
}
