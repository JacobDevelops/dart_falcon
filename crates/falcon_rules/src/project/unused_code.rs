//! `unused-code` — flag public top-level declarations in `lib/` whose name is
//! never referenced anywhere in the project. Port of dart_code_linter's
//! `check-unused-code`: a symbol referenced within its *own* file counts as used
//! (dcl records same-file references too), so only genuinely unreferenced public
//! declarations are flagged. Tuned to be low-false-positive by construction: when
//! in doubt (any same-name identifier in another file, any same-file reference
//! outside the declaration, any annotation, any export of the file) the
//! declaration is left unflagged.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{ProjectFile, ProjectRule};
use falcon_config::FalconConfig;
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::{canonical_or_lexical, detect_package, is_under_lib, resolve_directive_uri};

pub struct UnusedCode;

const NAME: &str = "unused-code";

/// A public top-level declaration considered for the unused check.
struct Candidate {
    name: String,
    span: Span,
    /// Full source span of the declaration (name, header, and body). Same-file
    /// occurrences inside this range are declaration-internal (the declared
    /// name, constructors/factories, member self-references) and do not count
    /// as usage; occurrences outside it do.
    decl_span: Span,
    kind: &'static str,
}

impl ProjectRule for UnusedCode {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze_project(&self, files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
        let pkg = detect_package(files);

        // name → set of file indices that mention it. Usage is detected by a
        // lexer identifier scan (not an AST walk): it never misses a reference —
        // type segments, `case` patterns, member access, combinators — and stays
        // correct even when a file's parse recovers partially. String-literal and
        // comment contents are excluded because the lexer tokenizes them apart.
        let mut mentions: HashMap<String, HashSet<usize>> = HashMap::new();
        for (idx, f) in files.iter().enumerate() {
            for name in collect_used_names(&f.source) {
                mentions.entry(name).or_default().insert(idx);
            }
        }

        // Files re-exported by another file: treat every public member as used
        // (the export surfaces the whole file's API).
        let exported = collect_exported_files(files, pkg.as_ref());

        let mut diags = Vec::new();
        for (idx, f) in files.iter().enumerate() {
            let canon = canonical_or_lexical(&f.path);
            if !is_under_lib(&canon, pkg.as_ref()) {
                continue;
            }
            if exported.contains(&canon) {
                continue;
            }
            // Error-recovery can leak method-body locals to the top level; don't
            // trust a file's own declaration list when it failed to parse.
            if f.has_parse_errors {
                continue;
            }
            let cands = candidates(&f.program);
            if cands.is_empty() {
                continue;
            }
            // Identifier ranges in the declaring file, used to detect same-file
            // references (dcl counts these as usage).
            let idents = identifier_ranges(&f.source);
            for cand in cands {
                let used_elsewhere = mentions
                    .get(&cand.name)
                    .is_some_and(|files| files.iter().any(|&j| j != idx));
                if used_elsewhere {
                    continue;
                }
                // Referenced within its own file, outside its own declaration
                // span → used (matches dcl, which records same-file references).
                let used_same_file = idents.iter().any(|&(offset, len)| {
                    f.source.get(offset..offset + len) == Some(cand.name.as_str())
                        && !(offset >= cand.decl_span.start && offset < cand.decl_span.end)
                });
                if used_same_file {
                    continue;
                }
                diags.push(Diagnostic::new(
                    NAME,
                    Severity::Warning,
                    format!(
                        "Public {} '{}' is never referenced anywhere in the project; \
                         remove it",
                        cand.kind, cand.name
                    ),
                    f.path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: cand.span.start,
                        end: cand.span.end,
                    },
                ));
            }
        }
        diags
    }
}

/// Whether an identifier names a public declaration (`_`-prefixed is private).
fn is_public(name: &str) -> bool {
    !name.starts_with('_')
}

/// The public top-level declarations of a program eligible for flagging.
/// Exemptions applied here: `main`, anything annotated, and unnamed extensions.
fn candidates(program: &Program) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut push =
        |name: &Identifier, kind: &'static str, anns: &[Annotation], decl_span: &Span| {
            if is_public(&name.name) && anns.is_empty() && name.name != "main" {
                out.push(Candidate {
                    name: name.name.clone(),
                    span: name.span.clone(),
                    decl_span: decl_span.clone(),
                    kind,
                });
            }
        };
    for decl in &program.declarations {
        let decl_span = decl.span();
        match decl {
            TopLevelDecl::Class(x) => push(&x.name, "class", &x.annotations, decl_span),
            TopLevelDecl::Mixin(x) => push(&x.name, "mixin", &x.annotations, decl_span),
            TopLevelDecl::MixinClass(x) => push(&x.name, "mixin class", &x.annotations, decl_span),
            TopLevelDecl::Enum(x) => push(&x.name, "enum", &x.annotations, decl_span),
            TopLevelDecl::ExtensionType(x) => {
                push(&x.name, "extension type", &x.annotations, decl_span)
            }
            TopLevelDecl::TypeAlias(x) => push(&x.name, "typedef", &x.annotations, decl_span),
            TopLevelDecl::Function(x) => push(&x.name, "declaration", &x.annotations, decl_span),
            // Extensions are invoked through their member names, not the
            // extension name, so a never-referenced name does not mean unused.
            // Without member-usage tracking they stay unflagged (low-FP).
            TopLevelDecl::Extension(_) => {}
            TopLevelDecl::Variable(x) => {
                for d in &x.declarators {
                    push(&d.name, "declaration", &x.annotations, decl_span);
                }
            }
            TopLevelDecl::Error(_) => {}
        }
    }
    out
}

/// Files that appear as the target of an `export` directive anywhere in the set.
fn collect_exported_files(
    files: &[ProjectFile],
    pkg: Option<&super::PackageInfo>,
) -> super::ReferenceSet {
    let mut refs = super::ReferenceSet::default();
    for f in files {
        let from = canonical_or_lexical(&f.path);
        for exp in &f.program.exports {
            if let Some(r) = resolve_directive_uri(&from, exp.uri.value.as_str(), pkg) {
                refs.insert(r);
            }
        }
    }
    refs
}

/// `(byte offset, byte length)` of every identifier a file *uses*. This is
/// deliberately over-inclusive — a spurious extra name only suppresses a flag,
/// keeping the rule low-false-positive — and, being purely lexical, it captures
/// references the AST walker would miss (type-name segments, `case` patterns,
/// member access, `show`/`hide` combinators) and survives partial parses.
///
/// Identifier tokens are taken directly; identifiers inside string-interpolation
/// regions (`$name`, `${expr}`) are extracted separately because the lexer folds
/// them into the enclosing string-literal token — without this, a symbol
/// referenced only inside an interpolation (a common Dart idiom) would look
/// unused and be falsely flagged. Keywords, plain string content, and comments
/// are distinct token kinds and stay excluded.
fn identifier_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for token in Lexer::new(source).tokenize() {
        match token.kind {
            TokenKind::Ident => out.push((token.offset, token.len)),
            TokenKind::StringLit => {
                push_interpolation_idents(token.text(source), token.offset, &mut out);
            }
            _ => {}
        }
    }
    out
}

/// The set of identifier names a file uses (see [`identifier_ranges`]).
fn collect_used_names(source: &str) -> HashSet<String> {
    identifier_ranges(source)
        .into_iter()
        .filter_map(|(offset, len)| source.get(offset..offset + len).map(str::to_string))
        .collect()
}

fn is_ident_start_byte(b: u8) -> bool {
    b == b'_' || b == b'$' || b.is_ascii_alphabetic()
}

fn is_ident_continue_byte(b: u8) -> bool {
    b == b'_' || b == b'$' || b.is_ascii_alphanumeric()
}

/// Append `(offset, len)` for each identifier inside the interpolation regions of
/// a string-literal token. `text` is the token's full source slice and `base` its
/// byte offset in the file. Raw strings (`r'…'`) never interpolate. Mirrors the
/// lexer's own `$`/`${…}` scanning and is over-inclusive (member names count
/// too), which only ever suppresses flags.
fn push_interpolation_idents(text: &str, base: usize, out: &mut Vec<(usize, usize)>) {
    let bytes = text.as_bytes();
    if bytes.first() == Some(&b'r') {
        return;
    }
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2, // escape: the next byte is literal
            b'$' => {
                i += 1;
                if bytes.get(i) == Some(&b'{') {
                    i += 1;
                    let mut depth = 1usize;
                    while i < bytes.len() && depth > 0 {
                        match bytes[i] {
                            b'{' => {
                                depth += 1;
                                i += 1;
                            }
                            b'}' => {
                                depth -= 1;
                                i += 1;
                            }
                            b'\\' => i += 2,
                            b if is_ident_start_byte(b) => {
                                let start = i;
                                while i < bytes.len() && is_ident_continue_byte(bytes[i]) {
                                    i += 1;
                                }
                                out.push((base + start, i - start));
                            }
                            _ => i += 1,
                        }
                    }
                } else if matches!(bytes.get(i), Some(&b) if is_ident_start_byte(b)) {
                    let start = i;
                    while i < bytes.len() && is_ident_continue_byte(bytes[i]) {
                        i += 1;
                    }
                    out.push((base + start, i - start));
                }
            }
            _ => i += 1,
        }
    }
}
