//! Flags Latin abbreviations in doc comments.
//!
//! Doc comments are read by API consumers who may not share the author's first
//! language, and abbreviations like `e.g.`, `i.e.`, `etc.`, and `et al.` are
//! easy to misread or mistranslate. Spelling them out ("for example", "that
//! is", "and so on") keeps generated documentation unambiguous. Only `///`
//! doc-comment lines are scanned, and the first occurrence of each abbreviation
//! on a line is reported.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidAbbreviationsInDocComments;

impl Rule for AvoidAbbreviationsInDocComments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        scan_source(ctx.source, &mut diags, ctx);
        RulePreparation::Diagnostics(diags)
    }
}

fn scan_source(source: &str, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // pyramid_lint's default abbreviation set. Matching is a case-sensitive
    // substring `contains`, reporting the first occurrence of each abbreviation
    // in a doc-comment line (mirrors `commentText.contains(abbreviation)` /
    // `indexOf` in avoid_abbreviations_in_doc_comments.dart).
    let abbreviations = ["e.g.", "i.e.", "etc.", "et al."];

    let mut byte_offset = 0;
    for line in source.lines() {
        if line.trim_start().starts_with("///") {
            for &abbr in &abbreviations {
                if let Some(index) = line.find(abbr) {
                    let start = byte_offset + index;
                    diags.push(ctx.diagnostic(
                        "avoid-abbreviations-in-doc-comments",
                        format!("Avoid abbreviation '{}' in doc comments", abbr),
                        DiagSpan {
                            start,
                            end: start + abbr.len(),
                        },
                    ));
                }
            }
        }

        // Move byte_offset to the next line (+1 for the newline character).
        byte_offset += line.len() + 1;
    }
}
