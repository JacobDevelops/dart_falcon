//! Attach library documentation to an explicit `library` directive.
//! Line docs stay grouped across blank lines and ordinary comments; declarations
//! report the doc token immediately before the first internal gap.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::{Annotation, AstChildren, Program};

#[derive(Default)]
pub struct DanglingLibraryDocComments;

impl Rule for DanglingLibraryDocComments {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let limit = first_directive(program, ctx)
            .map(|(offset, _)| offset)
            .or_else(|| program.declarations().map(|decl| decl.span().start).min())
            .unwrap_or(ctx.source().len());
        let comments = documentation_blocks(ctx, limit);
        let starts = dangling_starts(program, ctx, &comments);
        RulePreparation::Diagnostics(starts
    .into_iter()
    .map(|start| {
        ctx.diagnostic("dangling-library-doc-comments", "Dangling library doc comment. Add a 'library' directive or attach it to a declaration.", DiagSpan {
            start,
            end: start + 3,
        })
    })
    .collect())
    }
}

#[derive(Clone, Copy)]
struct DocBlock {
    start: usize,
    end: usize,
    end_line: usize,
    gap_start: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FirstDirective {
    Library,
    PartOf,
    Other,
}

fn dangling_starts(
    program: Program<'_>,
    ctx: &AnalyzeContext,
    comments: &[DocBlock],
) -> Vec<usize> {
    if let Some((offset, kind)) = first_directive(program, ctx) {
        if matches!(kind, FirstDirective::Library | FirstDirective::PartOf) {
            return Vec::new();
        }
        return attached_comment(ctx, comments, offset)
            .map(|(comment, _)| vec![comment.start])
            .unwrap_or_default();
    }

    let Some(keyword) = program.declarations().map(|decl| decl.span().start).min() else {
        return ctx
            .tokens()
            .filter(|token| token.kind() == falcon_syntax::TokenKind::DocComment)
            .map(|token| token.offset())
            .collect();
    };
    let Some((comment, offset)) = attached_comment(ctx, comments, keyword) else {
        return Vec::new();
    };
    if let Some(start) = comment.gap_start {
        return vec![start];
    }
    let mut end_line = comment.end_line;
    let mut scanned = comment.end;
    for token in ctx.tokens().filter(|token| {
        token.offset() >= comment.end
            && token.offset() < offset
            && matches!(
                token.kind(),
                falcon_syntax::TokenKind::LineComment | falcon_syntax::TokenKind::BlockComment
            )
    }) {
        let start_line = end_line
            + ctx.source()[scanned..token.offset()]
                .bytes()
                .filter(|b| *b == b'\n')
                .count();
        if start_line > end_line + 1 {
            return vec![comment.start];
        }
        scanned = token.offset() + token.len();
        end_line = start_line
            + ctx.source()[token.offset()..scanned]
                .bytes()
                .filter(|b| *b == b'\n')
                .count();
    }
    let declaration_line = end_line
        + ctx.source()[scanned..offset]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
    (declaration_line > end_line + 1)
        .then_some(vec![comment.start])
        .unwrap_or_default()
}

// Comments before the keyword win; otherwise use the last annotated token
// with preceding documentation, as the analyzer's metadata fallback does.
fn attached_comment<'a>(
    ctx: &AnalyzeContext,
    comments: &'a [DocBlock],
    keyword: usize,
) -> Option<(&'a DocBlock, usize)> {
    let mut comments = comments.iter().peekable();
    let mut candidate = None;
    for token in ctx.tokens().take_while(|token| token.offset() < keyword) {
        if token.is_trivia() {
            continue;
        }
        let mut preceding = None;
        while comments
            .peek()
            .is_some_and(|comment| comment.start < token.offset())
        {
            preceding = comments.next();
        }
        if token.kind() == falcon_syntax::TokenKind::At
            && let Some(comment) = preceding
        {
            candidate = Some((comment, token.offset()));
        }
    }
    comments
        .last()
        .map(|comment| (comment, keyword))
        .or(candidate)
}

fn first_directive(program: Program<'_>, ctx: &AnalyzeContext) -> Option<(usize, FirstDirective)> {
    let keyword = |annotations: AstChildren<'_, Annotation<'_>>, span| {
        crate::lint::declarations::first_token_after_annotations(
            annotations,
            span,
            ctx.tree(),
            ctx.source(),
        )
    };
    let mut directives = Vec::new();
    if let Some(directive) = program.library_directive() {
        directives.push((
            keyword(directive.annotations(), directive.span()),
            FirstDirective::Library,
        ));
    }
    if let Some(directive) = program.part_of_directive() {
        directives.push((
            keyword(directive.annotations(), directive.span()),
            FirstDirective::PartOf,
        ));
    }
    directives.extend(program.part_directives().map(|directive| {
        (
            keyword(directive.annotations(), directive.span()),
            FirstDirective::Other,
        )
    }));
    directives.extend(program.imports().map(|directive| {
        (
            keyword(directive.annotations(), directive.span()),
            FirstDirective::Other,
        )
    }));
    directives.extend(program.exports().map(|directive| {
        (
            keyword(directive.annotations(), directive.span()),
            FirstDirective::Other,
        )
    }));
    directives.into_iter().min_by_key(|(start, _)| *start)
}

fn documentation_blocks(ctx: &AnalyzeContext, limit: usize) -> Vec<DocBlock> {
    let mut blocks: Vec<DocBlock> = Vec::new();
    let mut scanned = 0;
    let mut line = 1;
    let mut metadata_end = 0;
    let mut last_doc_start = 0;
    for token in ctx.tokens() {
        if token.offset() >= limit {
            break;
        }
        if !token.is_trivia() {
            metadata_end = token.offset() + token.len();
        }
        if token.kind() != falcon_syntax::TokenKind::DocComment {
            continue;
        }
        let end = token.offset() + token.len();
        let start_line = line
            + ctx.source()[scanned..token.offset()]
                .bytes()
                .filter(|b| *b == b'\n')
                .count();
        let end_line = start_line
            + ctx.source()[token.offset()..end]
                .bytes()
                .filter(|b| *b == b'\n')
                .count();
        scanned = end;
        line = end_line;
        if let Some(previous) = blocks.last_mut()
            && previous.start >= metadata_end
            // Analyzer groups line docs across whitespace and ordinary comments.
            && ctx.source()[previous.start..].starts_with("///")
            && ctx.source()[token.offset()..].starts_with("///")
        {
            if start_line > previous.end_line + 1 && previous.gap_start.is_none() {
                previous.gap_start = Some(last_doc_start);
            }
            previous.end = end;
            previous.end_line = end_line;
        } else {
            blocks.push(DocBlock {
                start: token.offset(),
                end,
                end_line,
                gap_start: None,
            });
        }
        last_doc_start = token.offset();
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use falcon_diagnostics::Diagnostic;
    use std::path::PathBuf;

    fn diags(source: &str) -> Vec<Diagnostic> {
        let falcon_dart_parser::ParseResult { tree, .. } = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        DanglingLibraryDocComments.analyze(parsed.program(), &ctx)
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

    #[test]
    fn line_doc_groups_keep_the_first_internal_gap() {
        for source in [
            "/// first\n\n/// last\nimport 'dart:async';",
            "/// first\n// ordinary\n/// last\n\nclass C {}",
            "/// first\n\n/// last\nclass C {}",
        ] {
            let diagnostics = diags(source);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].span.start, 0);
        }
        let source = "/// first\n/// second\n\n/// last\nclass C {}";
        let diagnostics = diags(source);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].span.start,
            source.find("/// second").unwrap()
        );
    }

    #[test]
    fn comment_only_files_report_each_doc_token() {
        let diagnostics = diags("/// first\n/// second\n");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn ordinary_comments_inside_doc_group_are_not_following_comments() {
        assert!(diags("/// a\n/* x */ /// b\nclass C {}").is_empty());
    }

    #[test]
    fn interpolation_documentation_is_not_library_documentation() {
        let source = "@Anno('${/** internal */ 1}')\nclass C {}";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(diags(source).is_empty());
    }
}
