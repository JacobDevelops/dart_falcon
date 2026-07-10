//! `unused-code` — flag public top-level declarations in `lib/` whose name is
//! never referenced outside the declaring file. Port of dart_code_linter's
//! `check-unused-code`, tuned to be low-false-positive by construction: when in
//! doubt (any same-name identifier in another file, any annotation, any export
//! of the file) the declaration is left unflagged.

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
            for cand in candidates(&f.program) {
                let used_elsewhere = mentions
                    .get(&cand.name)
                    .is_some_and(|files| files.iter().any(|&j| j != idx));
                if used_elsewhere {
                    continue;
                }
                diags.push(Diagnostic::new(
                    NAME,
                    Severity::Warning,
                    format!(
                        "Public {} '{}' is never used outside its declaring file; \
                         make it private or remove it",
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
    let mut push = |name: &Identifier, kind: &'static str, anns: &[Annotation]| {
        if is_public(&name.name) && anns.is_empty() && name.name != "main" {
            out.push(Candidate {
                name: name.name.clone(),
                span: name.span.clone(),
                kind,
            });
        }
    };
    for decl in &program.declarations {
        match decl {
            TopLevelDecl::Class(x) => push(&x.name, "class", &x.annotations),
            TopLevelDecl::Mixin(x) => push(&x.name, "mixin", &x.annotations),
            TopLevelDecl::MixinClass(x) => push(&x.name, "mixin class", &x.annotations),
            TopLevelDecl::Enum(x) => push(&x.name, "enum", &x.annotations),
            TopLevelDecl::ExtensionType(x) => push(&x.name, "extension type", &x.annotations),
            TopLevelDecl::TypeAlias(x) => push(&x.name, "typedef", &x.annotations),
            TopLevelDecl::Function(x) => push(&x.name, "declaration", &x.annotations),
            // Extensions are invoked through their member names, not the
            // extension name, so a never-referenced name does not mean unused.
            // Without member-usage tracking they stay unflagged (low-FP).
            TopLevelDecl::Extension(_) => {}
            TopLevelDecl::Variable(x) => {
                for d in &x.declarators {
                    push(&d.name, "declaration", &x.annotations);
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

/// Collect every identifier a file *uses*, by lexing its source and keeping the
/// text of every identifier token. This is deliberately over-inclusive — a
/// spurious extra name only suppresses a flag, keeping the rule
/// low-false-positive — and, being purely lexical, it captures references the
/// AST walker would miss (type-name segments, `case` patterns, member access,
/// `show`/`hide` combinators) and survives partial parses. Keywords, string
/// literals, and comments are distinct token kinds, so they are excluded.
fn collect_used_names(source: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for token in Lexer::new(source).tokenize() {
        if token.kind == TokenKind::Ident {
            names.insert(token.text(source).to_string());
        }
    }
    names
}
