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
//! the current package from third-party packages. Imports inside `@docImport`
//! doc comments are not checked.
//!
//! Adopted from the official `directives_ordering` lint.

use std::cmp::Ordering;
use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct DirectivesOrdering;

const RULE: &str = "directives-ordering";

/// A namespace directive (import or export) in source order.
#[derive(Clone, Copy)]
struct Namespace<'a> {
    uri: &'a str,
    span: Span,
}

impl Rule for DirectivesOrdering {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let imports: Vec<Namespace> = program
            .imports
            .iter()
            .map(|import| Namespace {
                uri: &import.uri.value,
                span: import.span,
            })
            .collect();
        let exports: Vec<Namespace> = program
            .exports
            .iter()
            .map(|export| Namespace {
                uri: &export.uri.value,
                span: export.span,
            })
            .collect();

        let mut linted = HashSet::new();
        let mut diags = Vec::new();
        let mut report = |span: Span, message: String| {
            if linted.insert(span.start) {
                diags.push(ctx.diagnostic(RULE, message, span));
            }
        };

        // 'dart:' directives go first.
        for (directives, kind) in [(&imports, "imports"), (&exports, "exports")] {
            for directive in directives
                .iter()
                .skip_while(|d| is_dart(d.uri))
                .filter(|d| is_dart(d.uri))
            {
                report(
                    directive.span,
                    format!("Place 'dart:' {kind} before other {kind}."),
                );
            }
        }

        // 'package:' directives go before relative ones.
        for (directives, kind) in [(&imports, "imports"), (&exports, "exports")] {
            for directive in directives
                .iter()
                .filter(|d| !is_dart(d.uri))
                .skip_while(|d| is_absolute(d.uri))
                .filter(|d| is_package(d.uri))
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
        type Section = fn(&str) -> bool;
        let sections: [Section; 3] = [is_dart, |uri| !is_absolute(uri), is_package];
        for in_section in sections {
            for directives in [&imports, &exports] {
                let section: Vec<&Namespace> =
                    directives.iter().filter(|d| in_section(d.uri)).collect();
                for pair in section.windows(2) {
                    if compare_directives(pair[0].uri, pair[1].uri) == Ordering::Greater {
                        report(
                            pair[1].span,
                            "Sort directive sections alphabetically.".to_string(),
                        );
                    }
                }
            }
        }

        diags
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectiveKind {
    Import,
    Export,
    Part,
    Other,
}

fn is_dart(uri: &str) -> bool {
    uri.starts_with("dart:")
}

fn is_package(uri: &str) -> bool {
    uri.starts_with("package:")
}

fn is_absolute(uri: &str) -> bool {
    uri.contains(':')
}

/// Dart's `String.compareTo`: lexicographic over UTF-16 code units.
fn dart_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The analyzer's `compareDirectives`: two package URIs, or two absolute
/// paths, compare their first path segment before the remainder.
fn compare_directives(a: &str, b: &str) -> Ordering {
    if (!is_package(a) || !is_package(b)) && (!a.starts_with('/') || !b.starts_with('/')) {
        return dart_compare(a, b);
    }
    let (Some(index_a), Some(index_b)) = (a.find('/'), b.find('/')) else {
        return dart_compare(a, b);
    };
    dart_compare(&a[..index_a], &b[..index_b])
        .then_with(|| dart_compare(&a[index_a + 1..], &b[index_b + 1..]))
}
