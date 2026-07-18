//! Flags a redundant type annotation on an initializing formal or super parameter.
//!
//! In `C(int this.x)` or `C(int super.x)` the parameter's type is already fixed by
//! the field (or super-parameter) it initializes, so restating it is redundant and
//! risks drifting out of sync with the field's declared type. Detection runs over the
//! raw source because the parser only records a field/super formal when the token is
//! literally `this`/`super` with no preceding type. To stay conservative the rule
//! flags only a `this.`/`super.` immediately preceded by a bare type identifier — a
//! shape that in valid Dart occurs solely in a constructor parameter list — and skips
//! non-type words such as `final`, `covariant`, `return`, and `await`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;

pub struct TypeInitFormals;

impl Rule for TypeInitFormals {
    fn name(&self) -> &'static str {
        "type-init-formals"
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
            for kw in [&b"this"[..], &b"super"[..]] {
                if keyword_at(bytes, i, kw) && followed_by_dot(bytes, i + kw.len()) {
                    if preceded_by_type(bytes, i) {
                        diags.push(Diagnostic::new(
                            "type-init-formals",
                            Severity::Warning,
                            "Don't type annotate initializing formals.".to_string(),
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan {
                                start: i,
                                end: i + kw.len(),
                            },
                        ));
                    }
                    i += kw.len();
                    break;
                }
            }
            i += 1;
        }

        diags
    }
}

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
        q @ (b'\'' | b'"') => {
            let mut j = i + 1;
            while j < bytes.len() {
                match bytes[j] {
                    b'\\' => j += 2,
                    c if c == q => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(j)
        }
        _ => None,
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn keyword_at(bytes: &[u8], i: usize, kw: &[u8]) -> bool {
    bytes[i..].starts_with(kw)
        && (i == 0 || !is_ident_byte(bytes[i - 1]))
        && bytes
            .get(i + kw.len())
            .map(|b| !is_ident_byte(*b))
            .unwrap_or(true)
}

fn followed_by_dot(bytes: &[u8], mut i: usize) -> bool {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    bytes.get(i) == Some(&b'.')
}

// True when the token immediately before position `i` is a bare type identifier
// (not a statement/expression keyword and not a formal-parameter modifier). Only
// then is the `this.`/`super.` a *typed* initializing formal.
fn preceded_by_type(bytes: &[u8], i: usize) -> bool {
    let mut j = i;
    while j > 0 && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j == 0 || !is_ident_byte(bytes[j - 1]) {
        return false;
    }
    let end = j;
    while j > 0 && is_ident_byte(bytes[j - 1]) {
        j -= 1;
    }
    let word = &bytes[j..end];
    // Keywords that can legally sit just before `this`/`super` in an expression
    // or as an untyped formal modifier — none of these is a type annotation.
    const NON_TYPES: &[&[u8]] = &[
        b"return",
        b"await",
        b"yield",
        b"throw",
        b"in",
        b"is",
        b"as",
        b"case",
        b"else",
        b"do",
        b"final",
        b"covariant",
        b"required",
        b"var",
    ];
    !NON_TYPES.contains(&word)
}
