//! Attach library documentation to an explicit `library` directive.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct DanglingLibraryDocComments;

impl Rule for DanglingLibraryDocComments {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let comments = documentation_blocks(ctx.source);
        let starts = dangling_starts(program, ctx.source, &comments);
        starts
            .into_iter()
            .map(|start| {
                ctx.diagnostic("dangling-library-doc-comments", "Dangling library doc comment. Add a 'library' directive or attach it to a declaration.", DiagSpan {
                    start,
                    end: start + 3,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct DocBlock {
    start: usize,
    end_line: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FirstDirective {
    Library,
    PartOf,
    Other,
}

fn dangling_starts(program: &Program, source: &str, comments: &[DocBlock]) -> Vec<usize> {
    if let Some((offset, kind)) = first_directive(program) {
        if matches!(kind, FirstDirective::Library | FirstDirective::PartOf) {
            return Vec::new();
        }
        return comments
            .iter()
            .rev()
            .find(|comment| comment.start < offset)
            .map(|comment| vec![comment.start])
            .unwrap_or_default();
    }

    let first_declaration = program
        .declarations
        .iter()
        .map(|decl| decl.span().start)
        .min();
    let Some(offset) = first_declaration else {
        return comments.iter().map(|comment| comment.start).collect();
    };
    let Some(comment) = comments.iter().rev().find(|comment| comment.start < offset) else {
        return Vec::new();
    };
    let declaration_line = line_of(source, offset);
    (declaration_line > comment.end_line + 1)
        .then_some(vec![comment.start])
        .unwrap_or_default()
}

fn first_directive(program: &Program) -> Option<(usize, FirstDirective)> {
    let mut directives = Vec::new();
    if let Some(directive) = &program.library_directive {
        directives.push((directive.span.start, FirstDirective::Library));
    }
    if let Some(directive) = &program.part_of_directive {
        directives.push((directive.span.start, FirstDirective::PartOf));
    }
    directives.extend(
        program
            .part_directives
            .iter()
            .map(|directive| (directive.span.start, FirstDirective::Other)),
    );
    directives.extend(
        program
            .imports
            .iter()
            .map(|directive| (directive.span.start, FirstDirective::Other)),
    );
    directives.extend(
        program
            .exports
            .iter()
            .map(|directive| (directive.span.start, FirstDirective::Other)),
    );
    directives.into_iter().min_by_key(|(start, _)| *start)
}

fn documentation_blocks(source: &str) -> Vec<DocBlock> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let text = lines[index];
        let trimmed = text.trim_start();
        let indent = text.len() - trimmed.len();
        if trimmed.starts_with("///") {
            let start = offset + indent;
            let mut end_line = line;
            offset += text.len();
            line += 1;
            index += 1;
            while index < lines.len() && lines[index].trim_start().starts_with("///") {
                end_line = line;
                offset += lines[index].len();
                line += 1;
                index += 1;
            }
            blocks.push(DocBlock { start, end_line });
            continue;
        }
        if trimmed.starts_with("/**") {
            let start = offset + indent;
            let mut end_line = line;
            let mut closed = trimmed.contains("*/");
            offset += text.len();
            line += 1;
            index += 1;
            while !closed && index < lines.len() {
                closed = lines[index].contains("*/");
                end_line = line;
                offset += lines[index].len();
                line += 1;
                index += 1;
            }
            blocks.push(DocBlock { start, end_line });
            continue;
        }
        offset += text.len();
        line += 1;
        index += 1;
    }
    blocks
}

use crate::lint::expressions::line_of;

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn diags(source: &str) -> Vec<Diagnostic> {
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = parse(source);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax);
        DanglingLibraryDocComments.analyze(&program, &ctx)
    }

    #[test]
    fn crlf_span_does_not_drift() {
        let source = "// header\r\n/// Dangling doc.\r\n\r\nimport 'dart:async';\r\n";
        let diagnostics = diags(source);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "///"
        );
    }
}
