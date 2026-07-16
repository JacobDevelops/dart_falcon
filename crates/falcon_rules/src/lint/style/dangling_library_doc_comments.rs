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
    let lines: Vec<&str> = source.lines().collect();
    let mut offsets = Vec::with_capacity(lines.len());
    let mut acc = 0;
    for l in &lines {
        offsets.push(acc);
        acc += l.len() + 1;
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
