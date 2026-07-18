//! Flags angle-bracket text in doc comments that Markdown would render as an HTML tag.
//!
//! Dart renders doc comments as Markdown, so an unbacktick-quoted `<int>` or
//! `List<int>` is parsed as an HTML tag and silently dropped from the generated docs.
//! The rule scans `///` comment lines for `<name` sequences whose name is not a known
//! HTML tag and is followed by a tag-like character (`>`, `,`, `<`, or whitespace),
//! which distinguishes a stray generic from real markup or a `<https://...>` autolink.
//! Wrap such type references in backticks so they survive rendering. Fenced code blocks,
//! indented code, and inline code spans are skipped.

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
        // Iterate with `split_inclusive` so the offset advances by the real chunk
        // length: `lines()` drops a full `\r\n` but only one byte would be added
        // back, drifting every span in a CRLF file.
        for chunk in ctx.source.split_inclusive('\n') {
            let line = chunk.trim_end_matches(['\r', '\n']);
            let indent = line.len() - line.trim_start().len();
            let trimmed = &line[indent..];

            if let Some(content) = trimmed.strip_prefix("///") {
                let content_offset = byte_offset + indent + 3;
                if content.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                } else if !in_fence && !content.starts_with("    ") {
                    scan_content(content, content_offset, &file, &mut diags);
                }
            }

            byte_offset += chunk.len();
        }
        diags
    }
}

/// Report each `<tag` in `content` that resembles a generic type rather than a
/// known HTML tag, skipping regions inside inline code spans (backticks) and
/// inside `[...]` spans (Dartdoc source links, which may carry type arguments
/// such as `[List<int>]`).
fn scan_content(content: &str, base: usize, file: &str, diags: &mut Vec<Diagnostic>) {
    let bytes = content.as_bytes();
    let mut in_code = false;
    let mut in_ref = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'`' && !in_ref {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if !in_code && (b == b'[' || b == b']') {
            in_ref = b == b'[';
            i += 1;
            continue;
        }
        if in_code || in_ref || b != b'<' {
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
    "keygen",
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

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Text each diagnostic actually spans, sliced back out of the source. A
    /// wrong byte offset yields wrong text, so this checks spans, not just count.
    fn spanned(source: &str) -> Vec<&str> {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        UnintendedHtmlInDocComment
            .analyze(&program, &ctx)
            .into_iter()
            .map(|d| &source[d.span.start..d.span.end])
            .collect()
    }

    #[test]
    fn crlf_spans_do_not_drift() {
        // Every line after the first would drift by one byte per preceding line
        // if offsets were derived from `lines()` (which eats both CRLF bytes).
        let crlf = "/// Returns a List<int>.\r\n\
                    /// Wraps a Future<void>.\r\n\
                    /// Builds a Map<String, int>.\r\n\
                    /// Emits a Stream<Event>.\r\n\
                    class Api {}\r\n";
        assert_eq!(spanned(crlf), ["<int", "<void", "<String", "<Event"]);

        // Same content with LF endings must yield the same spanned text.
        let lf = crlf.replace("\r\n", "\n");
        assert_eq!(spanned(&lf), ["<int", "<void", "<String", "<Event"]);
    }

    #[test]
    fn dartdoc_square_bracket_references_are_not_html() {
        assert!(spanned("/// See [List<int>] for details.\nclass A {}").is_empty());
        assert!(spanned("/// The [Map<String, int>] result.\nclass A {}").is_empty());
        // The carve-out ends at the closing bracket.
        assert_eq!(
            spanned("/// See [List<int>] then Future<void>.\nclass A {}"),
            ["<void"]
        );
    }

    #[test]
    fn keygen_is_a_known_html_tag() {
        assert!(spanned("/// Renders a <keygen> control.\nclass A {}").is_empty());
    }
}
