//! Flags a `///` doc comment at the top of a file that is not attached to a
//! declaration (it precedes a directive, is separated by a blank line, or ends
//! the file) and so should document a `library` directive instead. Ported from
//! package:lints `dangling_library_doc_comments`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct DanglingLibraryDocComments;

impl Rule for DanglingLibraryDocComments {
    fn name(&self) -> &'static str {
        "dangling-library-doc-comments"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        if let Some(start) = dangling_doc_start(ctx.source) {
            diags.push(Diagnostic::new(
                "dangling-library-doc-comments",
                Severity::Warning,
                "Dangling library doc comment. Add a 'library' directive or attach it to a declaration.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start,
                    end: start + 3,
                },
            ));
        }
        diags
    }
}

/// Byte offset of the `///` that opens a dangling top-of-file doc block, if
/// any. Only the first significant construct in the file is considered.
fn dangling_doc_start(source: &str) -> Option<usize> {
    // Offsets come from `split_inclusive` chunks: `lines()` drops a full `\r\n`
    // but only one byte would be added back, drifting every span in a CRLF file.
    let mut lines: Vec<&str> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    let mut acc = 0;
    for chunk in source.split_inclusive('\n') {
        offsets.push(acc);
        lines.push(chunk.trim_end_matches(['\r', '\n']));
        acc += chunk.len();
    }

    // Skip leading blanks, ordinary `//` line comments, and `#!` shebangs.
    // Bail out at the first real code before any doc comment.
    let mut i = 0;
    loop {
        let line = lines.get(i)?;
        let t = line.trim_start();
        if t.is_empty() || (t.starts_with("//") && !t.starts_with("///")) || t.starts_with("#!") {
            i += 1;
            continue;
        }
        if t.starts_with("///") {
            break;
        }
        return None;
    }

    let doc_line = i;
    while lines
        .get(i)
        .is_some_and(|l| l.trim_start().starts_with("///"))
    {
        i += 1;
    }

    // Classify the first significant line after the doc block.
    let mut saw_blank = false;
    while lines.get(i).is_some_and(|l| l.trim_start().is_empty()) {
        saw_blank = true;
        i += 1;
    }

    let dangling = match lines.get(i) {
        None => true, // Nothing follows the doc comment.
        Some(next) => {
            let t = next.trim_start();
            let is_library = t == "library" || t == "library;" || t.starts_with("library ");
            if is_library {
                // Correct usage requires the comment to sit directly above it.
                saw_blank
            } else if saw_blank {
                true
            } else {
                t.starts_with("import ") || t.starts_with("export ") || t.starts_with("part ")
            }
        }
    };

    if dangling {
        let indent = lines[doc_line].len() - lines[doc_line].trim_start().len();
        Some(offsets[doc_line] + indent)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn diags(source: &str) -> Vec<Diagnostic> {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        DanglingLibraryDocComments.analyze(&program, &ctx)
    }

    #[test]
    fn crlf_span_does_not_drift() {
        // Three lines precede the doc comment, so a `lines()`-derived offset
        // would land three bytes early and no longer point at the `///`.
        let crlf = "// header\r\n\
                    // more\r\n\
                    // still more\r\n\
                    /// Dangling doc.\r\n\
                    \r\n\
                    import 'dart:async';\r\n";
        let d = diags(crlf);
        assert_eq!(d.len(), 1);
        assert_eq!(&crlf[d[0].span.start..d[0].span.end], "///");

        let lf = crlf.replace("\r\n", "\n");
        let d = diags(&lf);
        assert_eq!(d.len(), 1);
        assert_eq!(&lf[d[0].span.start..d[0].span.end], "///");
    }

    #[test]
    fn crlf_span_accounts_for_indentation() {
        let crlf = "// a\r\n\
                    // b\r\n\
                    \t/// Indented dangling doc.\r\n";
        let d = diags(crlf);
        assert_eq!(d.len(), 1);
        assert_eq!(&crlf[d[0].span.start..d[0].span.end], "///");
    }
}
