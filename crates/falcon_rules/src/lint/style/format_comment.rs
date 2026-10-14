//! Flags comments that are not formatted as complete sentences.
//!
//! Sentence-cased, terminated comments read consistently and signal a finished
//! thought rather than a scratch note. A comment should start with an uppercase
//! letter and end with `.`, `!`, `?`, or `:`. Consecutive comment lines are
//! joined into one block and split into sentences before checking, so a sentence
//! that wraps across several `///` lines is judged as a whole and continuation
//! lines are never flagged on their own. Both `//` line comments and `///` doc
//! comments are checked by default.
//!
//! ## Options
//!
//! `only_doc_comments` (bool, default: `false`) — when `true`, only `///` doc
//! comments are checked.
//!
//! `ignored_patterns` (list of regexes, default: `[]`) — a comment matching any
//! of these patterns is skipped entirely; invalid patterns are ignored.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use regex::Regex;

#[derive(Default)]
pub struct FormatComment;

struct CommentCfg {
    only_doc_comments: bool,
    ignored_patterns: Vec<Regex>,
}

fn comment_cfg(ctx: &AnalyzeContext) -> CommentCfg {
    let opts = crate::lint::options::for_rule(ctx, "format-comment");

    let only_doc_comments = opts
        .and_then(|o| o.get("only_doc_comments"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let ignored_patterns = crate::lint::options::strings(opts, "ignored_patterns")
        .into_iter()
        .flatten()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect();

    CommentCfg {
        only_doc_comments,
        ignored_patterns,
    }
}

impl Rule for FormatComment {
    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let cfg = comment_cfg(ctx);
        let tokens = collect_comment_tokens(ctx.source);
        for group in group_comments(&tokens) {
            check_group(group, &mut diags, ctx, &cfg);
        }
        diags
    }
}

struct CommentToken<'a> {
    start: usize,
    marker: &'static str,
    lexeme: &'a str,
    line: usize,
}

const PUNCTUATION: [char; 4] = ['.', '!', '?', ':'];

// A colon introduces a clause, not a new sentence.
const SENTENCE_TERMINATORS: [char; 2] = ['.', '|'];

fn collect_comment_tokens(source: &str) -> Vec<CommentToken<'_>> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1usize;

    while i < len {
        match bytes[i] {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                let start = i;
                let mut slashes = 0;
                while i < len && bytes[i] == b'/' {
                    slashes += 1;
                    i += 1;
                }
                // Four or more slashes mark a regular comment.
                let marker = if slashes == 3 { "///" } else { "//" };
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                tokens.push(CommentToken {
                    start,
                    marker,
                    lexeme: &source[start..i],
                    line,
                });
            }
            quote @ (b'"' | b'\'') => {
                i += 1;
                while i < len {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'\n' => {
                            line += 1;
                            i += 1;
                        }
                        byte if byte == quote => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }

    tokens
}

fn group_comments<'a, 'b>(tokens: &'b [CommentToken<'a>]) -> Vec<&'b [CommentToken<'a>]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for i in 1..tokens.len() {
        let prev = &tokens[i - 1];
        let cur = &tokens[i];
        let contiguous = cur.line == prev.line + 1 && cur.marker == prev.marker;
        if !contiguous {
            groups.push(&tokens[start..i]);
            start = i;
        }
    }
    if !tokens.is_empty() {
        groups.push(&tokens[start..]);
    }
    groups
}

fn check_group(
    group: &[CommentToken<'_>],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &CommentCfg,
) {
    let is_doc = group.first().map(|t| t.marker == "///").unwrap_or(false);
    if !is_doc && cfg.only_doc_comments {
        return;
    }

    let valid = if group.len() == 1 {
        has_valid_single_line(group[0].lexeme, group[0].marker, cfg)
    } else {
        has_valid_multiline(group, cfg)
    };
    if valid {
        return;
    }

    // Doc blocks report once; regular blocks report each non-ignored token.
    if is_doc {
        let start = group[0].start;
        diags.push(ctx.diagnostic(
            "format-comment",
            "Prefer formatting comments like sentences",
            DiagSpan {
                start,
                end: start + group[0].marker.len(),
            },
        ));
    } else {
        for tok in group {
            let trimmed = tok.lexeme[tok.marker.len()..].trim();
            if is_ignore_comment(trimmed) || is_ignored_pattern(trimmed, cfg) {
                continue;
            }
            diags.push(ctx.diagnostic(
                "format-comment",
                "Prefer formatting comments like sentences",
                DiagSpan {
                    start: tok.start,
                    end: tok.start + tok.marker.len(),
                },
            ));
        }
    }
}

fn has_valid_single_line(lexeme: &str, marker: &str, cfg: &CommentCfg) -> bool {
    let comment_text = &lexeme[marker.len()..];
    let text = comment_text.trim();
    if text.is_empty()
        || is_ignore_comment(text)
        || is_macros(text)
        || is_ignored_pattern(text, cfg)
    {
        return true;
    }
    is_valid_sentence(comment_text)
}

fn has_valid_multiline(group: &[CommentToken<'_>], cfg: &CommentCfg) -> bool {
    let text = extract_text(group, cfg);
    split_sentences(&text).iter().all(|s| is_valid_sentence(s))
}

fn extract_text(group: &[CommentToken<'_>], cfg: &CommentCfg) -> String {
    let mut result = String::new();
    let mut skip_fenced = false;
    for tok in group {
        let content = tok.lexeme.replace(tok.marker, "");
        if content.contains("```") {
            skip_fenced = !skip_fenced;
        } else if !skip_fenced && !should_skip(&content, cfg) {
            result.push_str(&content);
        }
    }
    result
}

fn should_skip(text: &str, cfg: &CommentCfg) -> bool {
    let trimmed = text.trim();
    is_macros(text) || is_ignore_comment(trimmed) || is_ignored_pattern(trimmed, cfg)
}

fn is_valid_sentence(sentence: &str) -> bool {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return true;
    }
    let upper = trimmed
        .chars()
        .next()
        .map(|c| !c.is_lowercase())
        .unwrap_or(true);
    let last_symbol = trimmed
        .chars()
        .next_back()
        .map(|c| PUNCTUATION.contains(&c))
        .unwrap_or(false);
    let has_empty_space = sentence.starts_with(' ');
    upper && last_symbol && has_empty_space
}

// The regex crate cannot express this zero-width lookbehind boundary.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if SENTENCE_TERMINATORS.contains(&c) {
            let next_is_ws_or_end = chars.peek().is_none_or(|(_, next)| next.is_whitespace());
            if next_is_ws_or_end {
                let end = pos + c.len_utf8();
                sentences.push(&text[start..end]);
                start = end;
            }
        }
    }
    if start < text.len() {
        sentences.push(&text[start..]);
    }
    sentences
}

fn is_ignore_comment(text: &str) -> bool {
    text.starts_with("ignore:")
        || text.starts_with("ignore_for_file:")
        || text.starts_with("falcon-ignore")
}

fn is_macros(text: &str) -> bool {
    match text.find("{@") {
        Some(open) => text.as_bytes()[open + 2..].contains(&b'}'),
        None => false,
    }
}

fn is_ignored_pattern(text: &str, cfg: &CommentCfg) -> bool {
    cfg.ignored_patterns.iter().any(|re| re.is_match(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_scan_skips_both_quote_styles_and_escaped_quotes() {
        let source = r#"'// not a comment' "// also not a comment" 'escaped \' // text' "escaped \" // text"
// First.
/// Second."#;
        let tokens = collect_comment_tokens(source);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].lexeme, "// First.");
        assert_eq!(tokens[0].marker, "//");
        assert_eq!(tokens[0].line, 2);
        assert_eq!(tokens[1].lexeme, "/// Second.");
        assert_eq!(tokens[1].marker, "///");
        assert_eq!(tokens[1].line, 3);
    }

    #[test]
    fn sentence_boundaries_preserve_unicode_and_colon_clauses() {
        assert_eq!(
            split_sentences(" Élan: lowercase clause. Next| Last."),
            [" Élan: lowercase clause.", " Next|", " Last."]
        );
        assert_eq!(
            split_sentences(" Value.without spaces"),
            [" Value.without spaces"]
        );
        assert!(split_sentences("").is_empty());
    }
}
