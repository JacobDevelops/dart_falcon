use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidAbbreviationsInDocComments;

impl Rule for AvoidAbbreviationsInDocComments {
    fn name(&self) -> &'static str {
        "avoid_abbreviations_in_doc_comments"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        scan_source(ctx.source, &mut diags, ctx);
        diags
    }
}

fn scan_source(source: &str, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let lines: Vec<&str> = source.lines().collect();

    // Abbreviations to flag (derived from corpus bad.dart)
    let abbreviations = [
        "impl", "func", "config", "repo", "param", "util", "var", "arg", "cfg", "e.g.", "i.e.",
        "etc.",
    ];

    let mut byte_offset = 0;
    for line in lines {
        if line.trim_start().starts_with("///") {
            let trimmed = line.trim_start();
            let doc_text = &trimmed[3..];

            // Check for abbreviations in the doc comment text
            for &abbr in &abbreviations {
                if contains_word(doc_text, abbr) {
                    // Emit diagnostic at the start of this line
                    diags.push(Diagnostic::new(
                        "avoid_abbreviations_in_doc_comments",
                        Severity::Warning,
                        format!("Avoid abbreviation '{}' in doc comments", abbr),
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: byte_offset,
                            end: byte_offset + line.len(),
                        },
                    ));
                    break; // Only one diagnostic per line
                }
            }
        }

        // Move byte_offset to the next line
        byte_offset += line.len() + 1; // +1 for newline character
    }
}

// Check if a word (abbreviation) appears as a whole word in text
fn contains_word(text: &str, word: &str) -> bool {
    let lower_text = text.to_lowercase();
    let lower_word = word.to_lowercase();
    let word_bytes = lower_word.as_bytes();
    let text_bytes = lower_text.as_bytes();

    for i in 0..=text_bytes.len().saturating_sub(word_bytes.len()) {
        if &text_bytes[i..i + word_bytes.len()] == word_bytes {
            // Check boundaries
            let before_ok = i == 0 || !is_word_char(text_bytes[i - 1]);
            let after_ok = i + word_bytes.len() >= text_bytes.len()
                || !is_word_char(text_bytes[i + word_bytes.len()]);

            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
}
