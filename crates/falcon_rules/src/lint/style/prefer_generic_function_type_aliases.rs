//! Flags old-style function typedefs (`prefer-generic-function-type-aliases`,
//! adopted from package:lints): `typedef int F(int x);` should be written with
//! the generic function type syntax `typedef F = int Function(int);`.
//!
//! The parser only accepts the modern `typedef Name = Type;` form, so an
//! old-style typedef never survives as a clean `TypeAliasDecl`. Detection is
//! therefore done over the raw source: a `typedef` whose declaration reaches an
//! opening `(` before any `=` is the old, non-generic form.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;

pub struct PreferGenericFunctionTypeAliases;

impl Rule for PreferGenericFunctionTypeAliases {
    fn name(&self) -> &'static str {
        "prefer-generic-function-type-aliases"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let bytes = ctx.source.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if let Some(next) = skip_trivia(bytes, i) {
                i = next;
                continue;
            }

            if matches_keyword(bytes, i, b"typedef") {
                let kw_start = i;
                i += "typedef".len();
                if old_style_ahead(bytes, i) {
                    diags.push(Diagnostic::new(
                        "prefer-generic-function-type-aliases",
                        Severity::Warning,
                        "Use the generic function type syntax in typedefs.".to_string(),
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: kw_start,
                            end: kw_start + "typedef".len(),
                        },
                    ));
                }
                continue;
            }
            i += 1;
        }

        diags
    }
}

// If `i` starts a comment or string literal, returns the offset just past it;
// otherwise `None`. Keeps the `typedef` scan from tripping over the keyword
// appearing inside string/comment text.
fn skip_trivia(bytes: &[u8], i: usize) -> Option<usize> {
    match bytes[i] {
        b'/' if bytes.get(i + 1) == Some(&b'/') => {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            Some(j)
        }
        b'/' if bytes.get(i + 1) == Some(&b'*') => {
            let mut j = i + 2;
            while j < bytes.len() && !(bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/')) {
                j += 1;
            }
            Some((j + 2).min(bytes.len()))
        }
        q @ (b'\'' | b'"') => Some(skip_string(bytes, i, q)),
        _ => None,
    }
}

// Triple-quoted strings are one literal, so their contents (which may contain a
// lone apostrophe or a `typedef` token) must be skipped as a whole.
fn skip_string(bytes: &[u8], start: usize, quote: u8) -> usize {
    let triple = bytes.get(start + 1) == Some(&quote) && bytes.get(start + 2) == Some(&quote);
    let delim = if triple { 3 } else { 1 };
    let mut j = start + delim;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            c if c == quote => {
                if !triple {
                    return j + 1;
                }
                if bytes[j..].starts_with(&[quote; 3]) {
                    return j + 3;
                }
                j += 1;
            }
            _ => j += 1,
        }
    }
    j
}

fn matches_keyword(bytes: &[u8], i: usize, kw: &[u8]) -> bool {
    if !bytes[i..].starts_with(kw) {
        return false;
    }
    let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
    let after_ok = bytes
        .get(i + kw.len())
        .map(|b| !is_ident_byte(*b))
        .unwrap_or(true);
    before_ok && after_ok
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

// Scan from just after `typedef` for the first meaningful delimiter, skipping
// comments and strings. `(` before `=`/`;` means the old, non-generic form.
fn old_style_ahead(bytes: &[u8], mut i: usize) -> bool {
    while i < bytes.len() {
        if let Some(next) = skip_trivia(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' => return true,
            b'=' | b';' | b'{' => return false,
            _ => i += 1,
        }
    }
    false
}
