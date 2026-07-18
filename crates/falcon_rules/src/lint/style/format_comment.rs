//! Flags comments not formatted as proper sentences. Ported from dart_code_linter's `format-comment`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use regex::Regex;

pub struct FormatComment;

/// Resolved options for one analysis pass.
struct CommentCfg {
    /// When true, only `///` doc comments are checked.
    only_doc_comments: bool,
    /// A comment whose text matches any of these is skipped entirely.
    ignored_patterns: Vec<Regex>,
}

fn comment_cfg(ctx: &AnalyzeContext) -> CommentCfg {
    let opts = crate::meta::meta_for("format-comment")
        .and_then(|m| ctx.rule_options(m.group, "format-comment"));

    let only_doc_comments = opts
        .and_then(|o| o.get("only_doc_comments"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Compile each pattern leniently: an invalid regex is ignored rather than
    // panicking on user config.
    let ignored_patterns = opts
        .and_then(|o| o.get("ignored_patterns"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| Regex::new(s).ok())
                .collect()
        })
        .unwrap_or_default();

    CommentCfg {
        only_doc_comments,
        ignored_patterns,
    }
}

impl Rule for FormatComment {
    fn name(&self) -> &'static str {
        "format-comment"
    }

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

/// One line comment token lifted straight from the source text.
struct CommentToken<'a> {
    /// Byte offset of the leading `/`.
    start: usize,
    /// Marker: `"///"` for doc comments, `"//"` for regular comments.
    marker: &'static str,
    /// The full lexeme, `//`/`///` slashes included, to end of line.
    lexeme: &'a str,
    /// 1-indexed source line the comment begins on.
    line: usize,
}

const PUNCTUATION: [char; 4] = ['.', '!', '?', ':'];

/// Terminators that end a "sentence" for the multiline split (dcl's
/// `[\.|:]` char class, which also treats `|` as a boundary).
const SENTENCE_TERMINATORS: [char; 3] = ['.', '|', ':'];

/// Scan raw source for `//` / `///` line comments, skipping strings and block
/// comments. This mirrors dart_code_linter operating on the analyzer token
/// stream: block (`/* */`) comments are not this rule's concern.
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
                // `///` is a doc comment; `////+` (four or more) is a regular
                // comment per Dart, matching dcl's `startsWith('///')` guard.
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
            b'"' => {
                i += 1;
                while i < len {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'\n' => {
                            line += 1;
                            i += 1;
                        }
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'\'' => {
                i += 1;
                while i < len {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'\n' => {
                            line += 1;
                            i += 1;
                        }
                        b'\'' => {
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

/// Group consecutive same-kind comment lines into one block, the way the Dart
/// analyzer coalesces a run of `///` lines into a single `Comment` node. A block
/// break happens on a type change or a non-comment gap (blank line / code).
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
    // Regular comments are only linted when `only_doc_comments` is off.
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

    if is_doc {
        // dcl reports one issue on the whole doc-comment node.
        let start = group[0].start;
        diags.push(diag(ctx, start, start + group[0].marker.len()));
    } else {
        // dcl reports one issue per non-ignored regular-comment token.
        for tok in group {
            let stripped = tok.lexeme.replacen(tok.marker, "", 1);
            let trimmed = stripped.trim();
            if is_ignore_comment(trimmed) || is_ignored_pattern(trimmed, cfg) {
                continue;
            }
            diags.push(diag(ctx, tok.start, tok.start + tok.marker.len()));
        }
    }
}

fn diag(ctx: &AnalyzeContext, start: usize, end: usize) -> Diagnostic {
    Diagnostic::new(
        "format-comment",
        Severity::Warning,
        "Prefer formatting comments like sentences",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start, end },
    )
}

/// A single-line comment is valid when it is empty, an `ignore:` directive, a
/// `{@...}` macro, matches an ignored pattern, or reads as one sentence.
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

/// A multi-line block is valid when every sentence in the concatenated body
/// (markers stripped, code fences / macros / ignore lines removed) is a valid
/// sentence. Crucially the block is joined *before* sentence-splitting, so a
/// sentence that wraps across `///` lines is judged as a whole — continuation
/// lines are never flagged on their own.
fn has_valid_multiline(group: &[CommentToken<'_>], cfg: &CommentCfg) -> bool {
    let text = extract_text(group, cfg);
    split_sentences(&text).iter().all(|s| is_valid_sentence(s))
}

/// Concatenate line contents (marker stripped), skipping fenced code blocks,
/// macro-only lines, and ignore/ignored-pattern lines.
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

/// A sentence is valid when it starts with a space (i.e. the marker is followed
/// by whitespace), its first non-space character is upper-case, and it ends in
/// sentence punctuation.
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

/// Split on the zero-width boundary *after* a `.`/`|`/`:` that is followed by
/// whitespace or end-of-text — a hand-rolled equivalent of dcl's lookbehind
/// `RegExp(r'(?<=([\.|:](?=\s|\n|$)))')`, which the `regex` crate cannot express.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for idx in 0..chars.len() {
        let (pos, c) = chars[idx];
        if SENTENCE_TERMINATORS.contains(&c) {
            let next_is_ws_or_end = chars
                .get(idx + 1)
                .map(|(_, nc)| nc.is_whitespace())
                .unwrap_or(true);
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
    // Dart analyzer directives plus falcon's own `falcon-ignore` / `falcon-ignore-all`
    // suppressions: none of these are prose, so the rule leaves them alone.
    text.starts_with("ignore:")
        || text.starts_with("ignore_for_file:")
        || text.starts_with("falcon-ignore")
}

fn is_macros(text: &str) -> bool {
    // `{@...}` dartdoc macro directive.
    match text.find("{@") {
        Some(open) => text.as_bytes()[open + 2..].contains(&b'}'),
        None => false,
    }
}

fn is_ignored_pattern(text: &str, cfg: &CommentCfg) -> bool {
    cfg.ignored_patterns.iter().any(|re| re.is_match(text))
}
