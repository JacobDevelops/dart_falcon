//! Report public top-level declarations that nothing references.
//!
//! Flags a public top-level declaration in `lib/` — a class, mixin, enum,
//! typedef, extension type, function, or variable — whose name appears nowhere
//! else in the project, neither in another file nor elsewhere in its own file.
//! An unreferenced public API is usually dead code that outlived its callers,
//! but removing something still in use is worse than missing one, so the rule is
//! tuned to avoid false positives: it exempts `main`, annotated declarations,
//! members of re-exported files, and unnamed extensions, treats any same-name
//! identifier anywhere as usage, and skips files that failed to parse. Private
//! (`_`-prefixed) declarations are out of scope. This is a cross-file rule: it
//! runs in the cross-file pass over the whole analyzed file set and is
//! configured under the top-level `cross-file` section rather than `linter`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::cst::*;

use super::{canonical_package_path, detect_package, is_under_lib, resolve_directive_uri};

#[derive(Default)]
pub struct UnusedCode;

const NAME: &str = "unused-code";

/// A public top-level declaration considered for the unused check.
struct Candidate<'a> {
    name: &'a str,
    span: Span,
    /// Full source span of the declaration (name, header, and body). Same-file
    /// occurrences inside this range are declaration-internal (the declared
    /// name, constructors/factories, member self-references) and do not count
    /// as usage; occurrences outside it do.
    decl_span: Span,
    kind: &'static str,
}

impl CrossFileRule for UnusedCode {
    fn analyze_project(
        &self,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let pkg = detect_package(files);

        // name → set of file indices that mention it. Usage is detected by a
        // lexer identifier scan (not a syntax-tree walk): it never misses a reference —
        // type segments, `case` patterns, member access, combinators — and stays
        // correct even when a file's parse recovers partially. String-literal and
        // comment contents are excluded because the lexer tokenizes them apart.
        let mut mentions: HashMap<&str, HashSet<usize>> = HashMap::new();
        for (idx, f) in files.iter().enumerate() {
            for name in collect_used_names(f.source(), f.tree()) {
                mentions.entry(name).or_default().insert(idx);
            }
        }

        // Files re-exported by another file: treat every public member as used
        // (the export surfaces the whole file's API).
        let exported = collect_exported_files(files, pkg.as_ref());

        let mut diags = Vec::new();
        for (idx, f) in files.iter().enumerate() {
            let canon = canonical_package_path(&f.path);
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
            let cands = candidates(f.program());
            if cands.is_empty() {
                continue;
            }
            // Identifier ranges in the declaring file, used to detect same-file
            // references (dcl counts these as usage).
            let idents: Vec<_> = identifier_ranges(f.source(), f.tree()).collect();
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
                    f.source().get(offset..offset + len) == Some(cand.name)
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
                    cand.span,
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
fn candidates<'a>(program: Program<'a>) -> Vec<Candidate<'a>> {
    let mut out = Vec::new();
    let mut push = |name: Identifier<'a>,
                    kind: &'static str,
                    anns: AstChildren<'a, Annotation<'a>>,
                    decl_span: Span| {
        if is_public(name.name()) && anns.is_empty() && name.name() != "main" {
            out.push(Candidate {
                name: name.name(),
                span: name.span(),
                decl_span,
                kind,
            });
        }
    };
    for decl in program.declarations() {
        let decl_span = decl.span();
        match decl {
            TopLevelDecl::Class(x) => push(x.name(), "class", x.annotations(), decl_span),
            TopLevelDecl::ClassTypeAlias(x) => push(x.name(), "class", x.annotations(), decl_span),
            TopLevelDecl::Mixin(x) => push(x.name(), "mixin", x.annotations(), decl_span),
            TopLevelDecl::MixinClass(x) => {
                push(x.name(), "mixin class", x.annotations(), decl_span)
            }
            TopLevelDecl::Enum(x) => push(x.name(), "enum", x.annotations(), decl_span),
            TopLevelDecl::ExtensionType(x) => {
                push(x.name(), "extension type", x.annotations(), decl_span)
            }
            TopLevelDecl::TypeAlias(x) => push(x.name(), "typedef", x.annotations(), decl_span),
            TopLevelDecl::Function(x) => push(x.name(), "declaration", x.annotations(), decl_span),
            // Extensions are invoked through their member names, not the
            // extension name, so a never-referenced name does not mean unused.
            // Without member-usage tracking they stay unflagged (low-FP).
            TopLevelDecl::Extension(_) => {}
            TopLevelDecl::Variable(x) => {
                for d in x.declarators() {
                    push(d.name(), "declaration", x.annotations(), decl_span);
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
    pkg: Option<&falcon_analyze::PackageIdentity>,
) -> super::ReferenceSet {
    let mut refs = super::ReferenceSet::default();
    for f in files {
        let from = canonical_package_path(&f.path);
        for exp in f.program().exports() {
            if let Some(r) = resolve_directive_uri(&from, &exp.uri().value(), pkg) {
                refs.insert(r);
            }
        }
    }
    refs
}

/// `(byte offset, byte length)` of every identifier a file *uses*. This is
/// deliberately over-inclusive — a spurious extra name only suppresses a flag,
/// keeping the rule low-false-positive — and, being purely lexical, it captures
/// references the syntax-tree walker would miss (type-name segments, `case` patterns,
/// member access, `show`/`hide` combinators) and survives partial parses.
///
/// Retained identifiers include both simple and braced string interpolations,
/// but exclude plain string contents and comments.
fn identifier_ranges<'a>(
    source: &'a str,
    tree: &'a SyntaxTree,
) -> impl Iterator<Item = (usize, usize)> + 'a {
    tree.identifier_tokens(source)
        .map(|token| (token.offset(), token.len()))
}

/// The set of identifier names a file uses (see [`identifier_ranges`]).
fn collect_used_names<'a>(source: &'a str, tree: &'a SyntaxTree) -> HashSet<&'a str> {
    identifier_ranges(source, tree)
        .filter_map(|(offset, len)| source.get(offset..offset + len))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::collect_used_names;

    #[cfg(unix)]
    #[test]
    fn virtual_symlinked_package_reports_unused_but_preserves_exports() {
        use std::path::PathBuf;

        use falcon_analyze::{CrossFileRule, ProjectFile};

        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let workspace = root.path().join("workspace");
        let sources = root.path().join("sources");
        std::fs::create_dir_all(package.join("bin")).unwrap();
        std::fs::create_dir_all(sources.join("src")).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: app\n").unwrap();
        std::os::unix::fs::symlink(&package, &workspace).unwrap();
        std::os::unix::fs::symlink(&sources, package.join("lib")).unwrap();
        let file = |path: PathBuf, source: &str| {
            let parsed = falcon_dart_parser::parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            ProjectFile {
                path,
                parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                    source: source.into(),
                    tree: parsed.tree,
                }),
                has_parse_errors: false,
            }
        };
        let dead = workspace.join("lib/src/dead.dart");
        let files = [
            file(
                workspace.join("bin/main.dart"),
                "export '../lib/src/api.dart';\nvoid main() {}\n",
            ),
            file(workspace.join("lib/src/api.dart"), "class Exported {}\n"),
            file(dead.clone(), "class Unused {}\n"),
        ];
        assert!(files.iter().all(|file| !file.path.exists()));

        let diagnostics = super::UnusedCode.analyze_project(
            &files,
            &falcon_config::FalconConfig::default(),
            None,
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].file_path, dead.to_string_lossy());
        assert!(diagnostics[0].message.contains("'Unused'"));
    }

    #[test]
    fn retained_interpolation_identifiers_exclude_literal_text_and_comments() {
        let source = r#"final text = '$simple ${call(nested /* ghost */)} ${"literal"}';"#;
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let names = collect_used_names(source, &parsed.tree);
        for name in ["text", "simple", "call", "nested"] {
            assert!(names.contains(name), "{name}: {names:?}");
        }
        for name in ["ghost", "literal"] {
            assert!(!names.contains(name), "{name}: {names:?}");
        }
    }

    #[test]
    fn contextual_identifiers_in_interpolations_count_as_references() {
        let source = r#"final text = '${type} ${show} ${get} ${set} ${on} ${hide}';"#;
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let names = collect_used_names(source, &parsed.tree);
        for name in ["type", "show", "get", "set", "on", "hide"] {
            assert!(names.contains(name), "{name}: {names:?}");
        }
    }

    #[test]
    fn unrelated_required_modifier_does_not_reference_required_declaration() {
        let source = "const required = 1; void unrelated({required int value}) {}";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let names = collect_used_names(source, &parsed.tree);
        assert!(!names.contains("required"), "{names:?}");
    }

    #[test]
    fn required_identifier_in_interpolation_counts_as_a_reference() {
        let source = r#"const required = 1; final text = '${required}';"#;
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let names = collect_used_names(source, &parsed.tree);
        assert!(names.contains("required"), "{names:?}");
    }

    #[test]
    fn raw_and_escaped_dollars_do_not_reference_identifiers() {
        let source = r#"final raw = r'$ghost'; final escaped = '\$other';"#;
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let names = collect_used_names(source, &parsed.tree);
        assert!(!names.contains("ghost"));
        assert!(!names.contains("other"));
    }
}
