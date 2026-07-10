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
        .and_then(|m| ctx.config.rule_options(m.group, "format-comment"));

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
        scan_source(ctx.source, &mut diags, ctx, &cfg);
        diags
    }
}

fn scan_source(source: &str, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &CommentCfg) {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Block comment — skip entirely
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            // Line comment (`//`) or doc comment (`///`) — check format
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                let comment_start = i;
                // Count the run of leading slashes; exactly three marks a doc comment.
                let mut slashes = 0;
                while i < len && bytes[i] == b'/' {
                    slashes += 1;
                    i += 1;
                }
                let is_doc = slashes == 3;

                // Capture the comment text (through end of line) for pattern matching.
                let content_start = i;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                let content = source[content_start..i].trim();

                let should_check = !cfg.only_doc_comments || is_doc;
                let ignored = cfg.ignored_patterns.iter().any(|re| re.is_match(content));

                if should_check
                    && !ignored
                    && let Some(first) = content.chars().next()
                    && first.is_ascii_lowercase()
                {
                    diags.push(Diagnostic::new(
                        "format-comment",
                        Severity::Warning,
                        "Comments should start with an uppercase letter",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: comment_start,
                            end: comment_start + slashes,
                        },
                    ));
                }
            }
            // Double-quoted string — skip contents
            b'"' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            // Single-quoted string — skip contents
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
}
