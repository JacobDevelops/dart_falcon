//! Flags angle-bracket text in `///` doc comments that Markdown would render as
//! an HTML tag (e.g. `List<int>` outside backticks). Ported from package:lints
//! `unintended_html_in_doc_comment`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UnintendedHtmlInDocComment;

impl Rule for UnintendedHtmlInDocComment {
    fn name(&self) -> &'static str {
        "unintended-html-in-doc-comment"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let file = ctx.file_path.to_string_lossy().into_owned();
        let mut diags = Vec::new();

        let mut byte_offset = 0;
        let mut in_fence = false;
        for line in ctx.source.lines() {
            let line_len = line.len();
            let indent = line_len - line.trim_start().len();
            let trimmed = &line[indent..];

            if let Some(content) = trimmed.strip_prefix("///") {
                let content_offset = byte_offset + indent + 3;
                if content.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                } else if !in_fence && !content.starts_with("    ") {
                    scan_content(content, content_offset, &file, &mut diags);
                }
            }

            byte_offset += line_len + 1;
        }
        diags
    }
}

/// Report each `<tag` in `content` that resembles a generic type rather than a
/// known HTML tag, skipping regions inside inline code spans (backticks).
fn scan_content(content: &str, base: usize, file: &str, diags: &mut Vec<Diagnostic>) {
    let bytes = content.as_bytes();
    let mut in_code = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'`' {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if in_code || b != b'<' {
            i += 1;
            continue;
        }

        let mut p = i + 1;
        if bytes.get(p) == Some(&b'/') {
            p += 1;
        }
        let name_start = p;
        while p < bytes.len() && bytes[p].is_ascii_alphanumeric() {
            p += 1;
        }
        if p == name_start {
            i += 1;
            continue;
        }
        // The char after the tag name must continue a tag/generic, not e.g. a
        // URL scheme (`<https://...>`).
        let follows_tag = matches!(bytes.get(p), Some(b'>' | b',' | b'<' | b' ' | b'\t'));
        let name = content[name_start..p].to_ascii_lowercase();
        if follows_tag && !HTML_TAGS.contains(&name.as_str()) {
            diags.push(Diagnostic::new(
                "unintended-html-in-doc-comment",
                Severity::Warning,
                "Angle brackets will be interpreted as HTML.",
                file.to_string(),
                DiagSpan {
                    start: base + i,
                    end: base + p,
                },
            ));
        }
        i = p;
    }
}

/// Standard HTML tag names; `<tag>` for any of these is intentional markup.
const HTML_TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "meta",
    "meter",
    "nav",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "param",
    "picture",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];
