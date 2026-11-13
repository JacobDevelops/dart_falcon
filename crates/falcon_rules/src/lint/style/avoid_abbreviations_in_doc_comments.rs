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
        scan_comments(&mut diags, ctx);
        RulePreparation::Diagnostics(diags)
    }
}

fn scan_comments(diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // pyramid_lint's default abbreviation set. Matching is a case-sensitive
    // substring `contains`, reporting the first occurrence of each abbreviation
    // in a doc-comment line (mirrors `commentText.contains(abbreviation)` /
    // `indexOf` in avoid_abbreviations_in_doc_comments.dart).
    let abbreviations = ["e.g.", "i.e.", "etc.", "et al."];

    for token in ctx.tree().comment_tokens(ctx.source()) {
        let line = token.text();
        if line.starts_with("///") && !line.starts_with("////") {
            for &abbr in &abbreviations {
                if let Some(index) = line.find(abbr) {
                    let start = token.offset() + index;
                    diags.push(ctx.diagnostic(format!("Avoid abbreviation `{abbr}` in doc comments."), DiagSpan {
                        start,
                        end: start + abbr.len(),
                    }));
                }
            }
        }
    }
}
