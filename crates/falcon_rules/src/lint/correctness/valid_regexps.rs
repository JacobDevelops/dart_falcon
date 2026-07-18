//! Disallow `RegExp` patterns that are structurally invalid.
//!
//! Flags a `RegExp` constructed from a string literal whose pattern is
//! unmistakably broken — unbalanced `(`/`)` groups, an unterminated `[...]`
//! character class, or a trailing backslash. Such a pattern throws a
//! `FormatException` the moment the `RegExp` is built, so catching it statically
//! turns a runtime crash into a lint. The check is deliberately conservative: it
//! inspects only literal, non-interpolated patterns and applies a structural
//! test rather than a full regex parse, so it never false-positives on the
//! JavaScript-flavored constructs (lookahead, backreferences, named groups) that
//! Dart's `RegExp` accepts. Interpolated patterns, and non-raw literals
//! containing backslash escapes, are skipped.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct ValidRegexps;

impl Rule for ValidRegexps {
    fn name(&self) -> &'static str {
        "valid-regexps"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn first_positional(args: &ArgList) -> Option<&Expr> {
    args.positional.first()
}

/// The literal pattern of a `RegExp` first argument, if it can be analyzed. Returns `None`
/// for non-literals, interpolated strings, or non-raw strings containing backslash escapes
/// (whose true pattern depends on Dart-level escape resolution — skipped to stay sound).
fn extractable_pattern(expr: &Expr) -> Option<String> {
    let Expr::StringLit(node) = expr else {
        return None;
    };
    let raw = &node.raw;
    let is_raw = raw.as_bytes().first() == Some(&b'r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let dlen = if rest.starts_with("'''") || rest.starts_with("\"\"\"") {
        3
    } else if rest.starts_with('\'') || rest.starts_with('"') {
        1
    } else {
        return None;
    };
    let closing = &rest[..dlen];
    if rest.len() < 2 * dlen || !rest[dlen..].ends_with(closing) {
        return None;
    }
    let content = &raw[prefix + dlen..raw.len() - dlen];
    if content.contains('$') {
        return None; // interpolated: pattern not statically known
    }
    if !is_raw && content.contains('\\') {
        return None; // Dart escapes would need resolving first
    }
    Some(content.to_string())
}

/// Conservative structural check: balanced `()`, terminated `[...]`, no trailing backslash.
fn is_structurally_valid(pattern: &str) -> bool {
    let bytes = pattern.as_bytes();
    let mut paren_depth: i32 = 0;
    let mut in_class = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return false; // trailing backslash
                }
                i += 2;
                continue;
            }
            b'[' if !in_class => in_class = true,
            b']' if in_class => in_class = false,
            b'(' if !in_class => paren_depth += 1,
            b')' if !in_class => {
                paren_depth -= 1;
                if paren_depth < 0 {
                    return false; // unmatched close paren
                }
            }
            _ => {}
        }
        i += 1;
    }
    paren_depth == 0 && !in_class
}

fn is_regexp_type(dart_type: &DartType) -> bool {
    matches!(dart_type, DartType::Named(n) if n.segments.last().is_some_and(|s| s.name == "RegExp"))
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn check(&mut self, args: &ArgList) {
        if let Some(arg) = first_positional(args)
            && let Some(pattern) = extractable_pattern(arg)
            && !is_structurally_valid(&pattern)
        {
            let span = arg.span();
            self.diags.push(Diagnostic::new(
                "valid-regexps",
                Severity::Warning,
                "Invalid regular expression: unbalanced groups or character class.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, args, .. } => {
                if let Expr::Ident(id) = callee.as_ref()
                    && id.name == "RegExp"
                {
                    self.check(args);
                }
            }
            Expr::New {
                dart_type, args, ..
            } if is_regexp_type(dart_type) => {
                self.check(args);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
