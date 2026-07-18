//! Flags abbreviations in doc comments. Ported from pyramid_lint's `avoid_abbreviations_in_doc_comments`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidAbbreviationsInDocComments;

impl Rule for AvoidAbbreviationsInDocComments {
    fn name(&self) -> &'static str {
        "avoid-abbreviations-in-doc-comments"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        scan_source(ctx.source, &mut diags, ctx);
        diags
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
                    diags.push(Diagnostic::new(
                        "avoid-abbreviations-in-doc-comments",
                        Severity::Warning,
                        format!("Avoid abbreviation '{}' in doc comments", abbr),
                        ctx.file_path.to_string_lossy().into_owned(),
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
