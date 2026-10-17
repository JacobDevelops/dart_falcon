//! Flags import and export directives that break the Effective Dart ordering.
//!
//! Directives are grouped into sections — `dart:` first, then `package:`, then
//! relative URIs — with exports in their own section after every import, and each
//! section sorted alphabetically. Every directive is reported at most once, with
//! the first broken convention in this order:
//!
//! ```dart
//! import 'package:a/a.dart';
//! import 'dart:async';          // flagged: 'dart:' imports go first
//! import 'package:c/c.dart';
//! import 'package:b/b.dart';    // flagged: sort sections alphabetically
//! import 'x.dart';
//! export 'y.dart';              // flagged: exports go after all imports
//! import 'package:d/d.dart';    // flagged: 'package:' before relative
//! ```
//!
//! All `package:` directives form one section; the analyzer does not separate
//! the current package from third-party packages. `@docImport`s of the library
//! documentation comment form their own sections, checked like imports.
//!
//! Adopted from the official `directives_ordering` lint.

use std::cmp::Ordering;
use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, library_doc_imports};
use falcon_syntax::ast::*;

use super::conditional_uri_does_not_exist::uri_value;

#[derive(Default)]
pub struct DirectivesOrdering;

const RULE: &str = "directives-ordering";

/// A namespace directive (import, export, or `@docImport`) in source order.
struct Namespace {
    /// The URI with escapes resolved, as the analyzer's `stringValue`.
    uri: String,
    span: Span,
}

impl Rule for DirectivesOrdering {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let imports: Vec<Namespace> = program
            .imports
            .iter()
            .map(|import| Namespace {
                uri: uri_value(&import.uri),
                span: import.span,
            })
            .collect();
        let exports: Vec<Namespace> = program
            .exports
            .iter()
            .map(|export| Namespace {
                uri: uri_value(&export.uri),
                span: export.span,
            })
            .collect();
        let doc_imports: Vec<Namespace> = library_doc_imports(program, ctx.source, ctx.syntax)
            .map(|import| Namespace {
                uri: uri_value(&import.literal),
                span: import.span,
            })
            .collect();
        let groups = [
            (&imports, "imports"),
            (&exports, "exports"),
            (&doc_imports, "@docImports"),
        ];

        let mut linted = HashSet::new();
        let mut diags = Vec::new();
        let mut report = |span: Span, message: String| {
            if linted.insert(span.start) {
                diags.push(ctx.diagnostic(RULE, message, span));
            }
        };

        // 'dart:' directives go first.
        for (directives, kind) in groups {
            for directive in directives
                .iter()
                .skip_while(|d| is_dart(d))
                .filter(|d| is_dart(d))
            {
                report(
                    directive.span,
                    format!("Place 'dart:' {kind} before other {kind}."),
                );
            }
        }

        // 'package:' directives go before relative ones.
        for (directives, kind) in groups {
            for directive in directives
                .iter()
                .filter(|d| !is_dart(d))
                .skip_while(|d| is_absolute(d))
                .filter(|d| is_package(d))
            {
                report(
                    directive.span,
                    format!("Place 'package:' {kind} before relative {kind}."),
                );
            }
        }

        // Exports form their own section after all imports: walking backwards,
        // skip trailing parts, then the trailing export block; any export left
        // precedes an import.
        let mut directives: Vec<(usize, DirectiveKind, Span)> = program
            .imports
            .iter()
            .map(|d| (d.span.start, DirectiveKind::Import, d.span))
            .chain(
                program
                    .exports
                    .iter()
                    .map(|d| (d.span.start, DirectiveKind::Export, d.span)),
            )
            .chain(
                program
                    .part_directives
                    .iter()
                    .map(|d| (d.span.start, DirectiveKind::Part, d.span)),
            )
            .chain(
                program
                    .library_directive
                    .iter()
                    .map(|d| (d.span.start, DirectiveKind::Other, d.span)),
            )
            .chain(
                program
                    .part_of_directive
                    .iter()
                    .map(|d| (d.span.start, DirectiveKind::Other, d.span)),
            )
            .collect();
        directives.sort_by_key(|(start, _, _)| *start);
        for (_, _, span) in directives
            .iter()
            .rev()
            .skip_while(|(_, kind, _)| *kind == DirectiveKind::Part)
            .skip_while(|(_, kind, _)| *kind == DirectiveKind::Export)
            .filter(|(_, kind, _)| *kind == DirectiveKind::Export)
        {
            report(
                *span,
                "Specify exports in a separate section after all imports.".to_string(),
            );
        }

        // Each section is sorted alphabetically.
        type Section = fn(&Namespace) -> bool;
        let sections: [Section; 3] = [is_dart, |d| !is_absolute(d), is_package];
        for in_section in sections {
            for (directives, _) in groups {
                let section: Vec<&Namespace> =
                    directives.iter().filter(|d| in_section(d)).collect();
                for pair in section.windows(2) {
                    if compare_directives(&pair[0].uri, &pair[1].uri) == Ordering::Greater {
                        report(
                            pair[1].span,
                            "Sort directive sections alphabetically.".to_string(),
                        );
                    }
                }
            }
        }

        RulePreparation::Diagnostics(diags)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectiveKind {
    Import,
    Export,
    Part,
    Other,
}

fn is_dart(directive: &Namespace) -> bool {
    directive.uri.starts_with("dart:")
}

fn is_package(directive: &Namespace) -> bool {
    directive.uri.starts_with("package:")
}

fn is_absolute(directive: &Namespace) -> bool {
    directive.uri.contains(':')
}

/// Dart's `String.compareTo`: lexicographic over UTF-16 code units.
fn dart_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The analyzer's `compareDirectives`: two package URIs, or URIs where either
/// is an absolute path, compare their first path segment before the remainder.
fn compare_directives(a: &str, b: &str) -> Ordering {
    let both_package = a.starts_with("package:") && b.starts_with("package:");
    if !both_package && !a.starts_with('/') && !b.starts_with('/') {
        return dart_compare(a, b);
    }
    let (Some(index_a), Some(index_b)) = (a.find('/'), b.find('/')) else {
        return dart_compare(a, b);
    };
    dart_compare(&a[..index_a], &b[..index_b])
        .then_with(|| dart_compare(&a[index_a + 1..], &b[index_b + 1..]))
}

#[cfg(test)]
mod tests {
    use super::{compare_directives, uri_value};
    use falcon_analyze::library_doc_imports;

    #[test]
    fn doc_imports_use_literal_segments_and_import_spans() {
        let source = "/// @docImport r'dart:' '\\x63ollection';\n/// @docImport r'dart:' '\\x61sync';\nlibrary;\n";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
            ..
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let imports: Vec<_> = library_doc_imports(&program, source, &syntax).collect();
        assert_eq!(imports.len(), 2);
        assert_eq!(uri_value(&imports[0].literal), "dart:collection");
        assert_eq!(uri_value(&imports[1].literal), "dart:async");
        assert_eq!(
            &source[imports[1].span.start..imports[1].span.end],
            "Import r'dart:' '\\x61sync';"
        );
    }

    #[test]
    fn doc_import_spans_preserve_tabs_crlf_and_block_offsets() {
        let source = "\t/**\r\n\t * @docImport r'dart:collection';\r\n\t * @docImport 'dart:async';\r\n\t */\r\nlibrary;\r\n";
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = falcon_dart_parser::parse(source);
        let imports: Vec<_> = library_doc_imports(&program, source, &syntax).collect();
        assert_eq!(imports.len(), 2);
        assert_eq!(
            &source[imports[0].span.start..imports[0].span.end],
            "Import r'dart:collection';"
        );
        assert_eq!(
            &source[imports[1].span.start..imports[1].span.end],
            "Import 'dart:async';"
        );
    }

    #[test]
    fn doc_import_keyword_requires_a_space_and_maps_extra_whitespace() {
        let source = "/// @docImport\t'dart:io';\n/// @docImport   'dart:async';\nlibrary;\n";
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = falcon_dart_parser::parse(source);
        let imports: Vec<_> = library_doc_imports(&program, source, &syntax).collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].span.start, source.rfind("Import").unwrap() + 2);
        assert_eq!(imports[0].span.end - imports[0].span.start, 20);
    }

    #[test]
    fn doc_import_span_excludes_trailing_comment_trivia() {
        let source = "/// @docImport 'dart:async'; /* trailing trivia */\r\nlibrary;\r\n";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
            ..
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let imports: Vec<_> = library_doc_imports(&program, source, &syntax).collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(
            &source[imports[0].span.start..imports[0].span.end],
            "Import 'dart:async';"
        );
    }

    #[test]
    fn doc_imports_require_attachment_to_library_directive() {
        for source in [
            "/// @docImport 'dart:async';\n/** replacement */\nlibrary;",
            "/** @docImport 'dart:async'; */\n/// replacement\nlibrary;",
        ] {
            let falcon_dart_parser::ParseResult {
                program, syntax, ..
            } = falcon_dart_parser::parse(source);
            assert_eq!(
                library_doc_imports(&program, source, &syntax).count(),
                0,
                "{source}"
            );
        }
    }

    #[test]
    fn package_comparison_uses_package_name_then_path() {
        assert!(compare_directives("package:http_parser/a.dart", "package:http/b.dart").is_gt());
        assert!(compare_directives("package:http/a.dart", "package:http/b.dart").is_lt());
        assert!(compare_directives("\u{10000}", "\u{e000}").is_lt());
    }
}
