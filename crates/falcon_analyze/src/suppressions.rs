//! Inline diagnostic suppression via `// falcon-ignore <path>: <reason>` /
//! `// falcon-ignore-all <path>: <reason>` comments, modelled on Biome's
//! `// biome-ignore lint/<group>/<rule>: <explanation>` shape.
//!
//! `<path>` is `lint/<group>/<rule>` for file rules and
//! `cross-file/<group>/<rule>` for cross-file rules (the legacy `project/…`
//! section is still accepted). A reason is **required**; one rule per
//! comment; consecutive suppression-only comment lines stack and all apply to
//! the next line of code (Biome semantics). Falcon does **not** read Dart's own
//! `// ignore:` / `// ignore_for_file:` comments — those belong to the analyzer.
//!
//! Comments are read from a real lex pass (not a naive line scan), so a
//! suppression phrase sitting inside a string literal is never mistaken for a
//! real directive — the lexer tokenizes it as part of the `StringLit`.
//!
//! A malformed comment (missing reason, bad path, unknown rule, or a rule under
//! the wrong group/section) does **not** suppress; instead it surfaces a
//! [`MALFORMED_SUPPRESSION`] diagnostic so the mistake is visible. That rule is
//! internal — it is deliberately absent from `meta.rs`/`falcon.json`, and
//! `apply_severities` leaves diagnostics whose rule has no metadata untouched.

use std::collections::{HashMap, HashSet};

use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::token::{Token, TokenKind};

/// Rule name carried by the diagnostics emitted for malformed suppression
/// comments. Internal: not registered in `meta.rs`, not configurable, and never
/// itself suppressible.
pub const MALFORMED_SUPPRESSION: &str = "malformed-suppression";

/// Maps a (possibly-legacy) rule name to its `(canonical_name, group,
/// is_cross_file)` metadata, used to validate a suppression path. Returning the
/// canonical name lets a suppression written with an old rule id still match the
/// diagnostic's canonical rule. Supplied by callers because `falcon_analyze`
/// does not depend on `falcon_rules` (which owns the rule table).
pub type RuleLookup = fn(&str) -> Option<(&'static str, &'static str, bool)>;

/// Parsed suppression directives for a single source file.
pub struct FileSuppressions {
    /// Rule names suppressed everywhere in the file (`falcon-ignore-all`).
    all_file: HashSet<String>,
    /// Rule names suppressed on a specific 0-based line (`falcon-ignore`).
    by_line: HashMap<u32, HashSet<String>>,
    /// Byte offset of the start of each 0-based line, for offset→line lookup.
    line_starts: Vec<usize>,
    /// `malformed-suppression` diagnostics collected while parsing.
    diagnostics: Vec<Diagnostic>,
}

impl FileSuppressions {
    /// Lex `source` once and collect every `falcon-ignore` directive it
    /// contains, validating each path against `lookup`. `file_path` is used only
    /// to stamp the malformed-suppression diagnostics.
    pub fn parse(source: &str, file_path: &str, lookup: RuleLookup) -> Self {
        let mut me = Self {
            all_file: HashSet::new(),
            by_line: HashMap::new(),
            line_starts: compute_line_starts(source),
            diagnostics: Vec::new(),
        };

        // Fast path: the lexer pass only earns its keep on files that actually
        // mention the directive. A match inside a string literal merely triggers
        // the pass, which then correctly ignores it — so this stays correct.
        if !source.contains("falcon-ignore") {
            return me;
        }

        // Line of the most recent non-trivia token; distinguishes a trailing
        // `// falcon-ignore` (code precedes it → same line) from a standalone
        // one (nothing before it → applies to the next code line).
        let mut last_code_line: Option<u32> = None;
        // Standalone directives awaiting the next code line (Biome stacking).
        let mut pending: Vec<String> = Vec::new();

        for token in Lexer::new(source).tokenize() {
            let line = line_for_offset(&me.line_starts, token.offset);

            if token.is_trivia() {
                // Only line-form comments carry directives: `//`/`///`. Block
                // comments (`/* */`) and block doc comments (`/** */`) do not,
                // even though the latter lexes as `DocComment`.
                let is_line_directive = matches!(token.kind, TokenKind::LineComment)
                    || (token.kind == TokenKind::DocComment
                        && token.text(source).starts_with("///"));
                if is_line_directive && let Some((keyword, rest)) = classify(token.text(source)) {
                    match validate(keyword, rest, lookup) {
                        Ok(rule) => {
                            if keyword == KW_ALL {
                                me.all_file.insert(rule);
                            } else if last_code_line == Some(line) {
                                // Trailing comment: suppress its own line.
                                me.by_line.entry(line).or_default().insert(rule);
                            } else {
                                // Standalone: defer to the next code line.
                                pending.push(rule);
                            }
                        }
                        Err(message) => me
                            .diagnostics
                            .push(malformed_diag(&token, source, file_path, message)),
                    }
                }
                continue;
            }

            // First code token on a new line flushes any stacked standalone
            // directives onto this line.
            if last_code_line != Some(line) {
                if !pending.is_empty() {
                    let entry = me.by_line.entry(line).or_default();
                    entry.extend(pending.drain(..));
                }
                last_code_line = Some(line);
            }
        }

        me
    }

    /// The 0-based line a byte offset falls on (used for a diagnostic's span
    /// start). Shared here so callers don't recompute a line table.
    pub fn line_for_offset(&self, offset: usize) -> u32 {
        line_for_offset(&self.line_starts, offset)
    }

    /// Whether `rule` is suppressed on the given 0-based `line`.
    pub fn is_suppressed(&self, rule: &str, line: u32) -> bool {
        self.all_file.contains(rule)
            || self
                .by_line
                .get(&line)
                .is_some_and(|rules| rules.contains(rule))
    }

    /// True when the file carries no *valid* directives (skip the filter pass).
    /// Malformed-suppression diagnostics are reported separately and do not
    /// count here.
    pub fn is_empty(&self) -> bool {
        self.all_file.is_empty() && self.by_line.is_empty()
    }

    /// Diagnostics for malformed suppression comments found while parsing.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consume the parse result, yielding its malformed-suppression diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

const KW_ALL: &str = "falcon-ignore-all";
const KW_LINE: &str = "falcon-ignore";

/// Recognize a `falcon-ignore` / `falcon-ignore-all` line comment, returning the
/// matched keyword and the `<path>: <reason>` remainder. The `-all` form is
/// tested first since it is a prefix superset of the line form.
fn classify(text: &str) -> Option<(&'static str, &str)> {
    let body = text.trim_start_matches('/').trim_start();
    if let Some(rest) = strip_keyword(body, KW_ALL) {
        Some((KW_ALL, rest))
    } else {
        strip_keyword(body, KW_LINE).map(|rest| (KW_LINE, rest))
    }
}

/// Strip `keyword` from the front of `body` only when it stands as a whole word
/// (followed by whitespace or end of comment), returning the trimmed remainder.
fn strip_keyword<'a>(body: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = body.strip_prefix(keyword)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest.trim_start())
    } else {
        None
    }
}

/// Validate a directive's `<path>: <reason>` remainder. Returns the bare rule
/// name to suppress on success, or a human-readable message describing why the
/// comment is malformed (and therefore does not suppress).
fn validate(keyword: &str, rest: &str, lookup: RuleLookup) -> Result<String, String> {
    let (path, reason) = match rest.split_once(':') {
        Some((p, r)) => (p.trim(), r.trim()),
        None => (rest.trim(), ""),
    };

    if reason.is_empty() {
        let shown = if path.is_empty() {
            "lint/<group>/<rule>"
        } else {
            path
        };
        return Err(format!(
            "{keyword} requires a reason: // {keyword} {shown}: <why>"
        ));
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() != 3 || !matches!(parts[0], "lint" | "cross-file" | "project") {
        return Err(format!(
            "malformed {keyword} path '{path}'; expected lint/<group>/<rule> or cross-file/<group>/<rule>"
        ));
    }
    // Normalize the legacy `project/…` section to its canonical `cross-file/…`.
    let section = if parts[0] == "project" {
        "cross-file"
    } else {
        parts[0]
    };
    let (group, rule) = (parts[1], parts[2]);

    match lookup(rule) {
        None => Err(format!("unknown rule '{rule}' in {keyword} path '{path}'")),
        Some((canonical, correct_group, is_cross_file)) => {
            let correct_section = if is_cross_file { "cross-file" } else { "lint" };
            if group != correct_group || section != correct_section {
                Err(format!(
                    "suppression path is {correct_section}/{correct_group}/{canonical}"
                ))
            } else {
                // Record the canonical id so a suppression written with a legacy
                // alias still matches the diagnostic's canonical rule name.
                Ok(canonical.to_string())
            }
        }
    }
}

/// Build a `malformed-suppression` warning anchored to the offending comment.
fn malformed_diag(token: &Token, source: &str, file_path: &str, message: String) -> Diagnostic {
    let start = token.offset;
    let end = start + token.text(source).len();
    Diagnostic::new(
        MALFORMED_SUPPRESSION,
        Severity::Warning,
        message,
        file_path,
        Span { start, end },
    )
}

fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn line_for_offset(line_starts: &[usize], offset: usize) -> u32 {
    match line_starts.binary_search(&offset) {
        Ok(i) => i as u32,
        Err(i) => (i - 1) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test lookup mirroring a slice of the real rule table: a file rule and a
    /// cross-file rule, enough to exercise group/section validation.
    fn lookup(name: &str) -> Option<(&'static str, &'static str, bool)> {
        // (canonical_name, group, is_cross_file). `no_empty_block` is a legacy
        // alias that canonicalizes to `no-empty-block`, exercising the
        // alias-aware suppression path.
        match name {
            "avoid-dynamic" => Some(("avoid-dynamic", "suspicious", false)),
            "no-equal-arguments" => Some(("no-equal-arguments", "suspicious", false)),
            "unused-files" => Some(("unused-files", "correctness", true)),
            "no_empty_block" | "no-empty-block" => Some(("no-empty-block", "suspicious", false)),
            _ => None,
        }
    }

    fn parse(source: &str) -> FileSuppressions {
        FileSuppressions::parse(source, "test.dart", lookup)
    }

    fn diag_rules(s: &FileSuppressions) -> Vec<&str> {
        s.diagnostics().iter().map(|d| d.rule).collect()
    }

    #[test]
    fn same_line_suppression() {
        let s = parse("dynamic x; // falcon-ignore lint/suspicious/avoid-dynamic: legacy\n");
        assert!(s.is_suppressed("avoid-dynamic", 0));
        assert!(!s.is_suppressed("avoid-dynamic", 1));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn next_line_suppression() {
        let s = parse("// falcon-ignore lint/suspicious/avoid-dynamic: legacy\ndynamic x;\n");
        assert!(s.is_suppressed("avoid-dynamic", 1));
        assert!(!s.is_suppressed("avoid-dynamic", 0));
    }

    #[test]
    fn stacked_standalone_comments_apply_to_next_code_line() {
        let src = "// falcon-ignore lint/suspicious/avoid-dynamic: a\n\
                   // falcon-ignore lint/suspicious/no-equal-arguments: b\n\
                   dynamic x = f(1, 1);\n";
        let s = parse(src);
        assert!(s.is_suppressed("avoid-dynamic", 2));
        assert!(s.is_suppressed("no-equal-arguments", 2));
    }

    #[test]
    fn all_file_applies_anywhere() {
        let s = parse(
            "void f() {}\n// falcon-ignore-all lint/suspicious/avoid-dynamic: sweep\ndynamic y;\n",
        );
        assert!(s.is_suppressed("avoid-dynamic", 0));
        assert!(s.is_suppressed("avoid-dynamic", 2));
        assert!(s.is_suppressed("avoid-dynamic", 999));
    }

    #[test]
    fn cross_file_rule_path() {
        let s = parse("// falcon-ignore-all cross-file/correctness/unused-files: generated\n");
        assert!(s.is_suppressed("unused-files", 0));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn legacy_project_section_still_suppresses() {
        // The pre-rename `project/…` section is accepted as a deprecated alias
        // for `cross-file/…` and must still suppress without a diagnostic.
        let s = parse("// falcon-ignore-all project/correctness/unused-files: generated\n");
        assert!(s.is_suppressed("unused-files", 0));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn legacy_alias_suppression_matches_canonical_rule() {
        // A suppression written with the old `no_empty_block` id must suppress
        // the canonical `no-empty-block` diagnostic.
        let s = parse("void f() {} // falcon-ignore lint/suspicious/no_empty_block: legacy\n");
        assert!(s.is_suppressed("no-empty-block", 0));
        assert!(!s.is_suppressed("no_empty_block", 0));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn missing_reason_reports_and_does_not_suppress() {
        let s = parse("dynamic x; // falcon-ignore lint/suspicious/avoid-dynamic\n");
        assert!(!s.is_suppressed("avoid-dynamic", 0));
        assert_eq!(diag_rules(&s), vec![MALFORMED_SUPPRESSION]);
        assert!(s.diagnostics()[0].message.contains("requires a reason"));
    }

    #[test]
    fn empty_reason_reports() {
        let s = parse("dynamic x; // falcon-ignore lint/suspicious/avoid-dynamic:   \n");
        assert!(!s.is_suppressed("avoid-dynamic", 0));
        assert_eq!(diag_rules(&s), vec![MALFORMED_SUPPRESSION]);
    }

    #[test]
    fn wrong_group_reports_correct_path() {
        let s = parse("// falcon-ignore lint/style/avoid-dynamic: x\ndynamic y;\n");
        assert!(!s.is_suppressed("avoid-dynamic", 1));
        assert_eq!(diag_rules(&s), vec![MALFORMED_SUPPRESSION]);
        assert!(
            s.diagnostics()[0]
                .message
                .contains("suppression path is lint/suspicious/avoid-dynamic")
        );
    }

    #[test]
    fn wrong_section_reports_correct_path() {
        // Cross-file rule referenced under `lint/` instead of `cross-file/`.
        let s = parse("// falcon-ignore-all lint/correctness/unused-files: x\n");
        assert!(!s.is_suppressed("unused-files", 0));
        assert!(
            s.diagnostics()[0]
                .message
                .contains("suppression path is cross-file/correctness/unused-files")
        );
    }

    #[test]
    fn unknown_rule_reports() {
        let s = parse("// falcon-ignore lint/suspicious/not-a-rule: x\ndynamic y;\n");
        assert_eq!(diag_rules(&s), vec![MALFORMED_SUPPRESSION]);
        assert!(
            s.diagnostics()[0]
                .message
                .contains("unknown rule 'not-a-rule'")
        );
    }

    #[test]
    fn malformed_path_reports() {
        let s = parse("// falcon-ignore avoid-dynamic: x\ndynamic y;\n");
        assert_eq!(diag_rules(&s), vec![MALFORMED_SUPPRESSION]);
        assert!(s.diagnostics()[0].message.contains("malformed"));
    }

    #[test]
    fn dart_ignore_is_not_read() {
        // Falcon no longer honors Dart's own `// ignore:` comments.
        let s = parse("dynamic x; // ignore: avoid-dynamic\n");
        assert!(s.is_empty());
        assert!(!s.is_suppressed("avoid-dynamic", 0));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn suppression_inside_string_literal_does_not_count() {
        let src = "var s = '// falcon-ignore lint/suspicious/avoid-dynamic: x';\ndynamic x;\n";
        let s = parse(src);
        assert!(s.is_empty());
        assert!(!s.is_suppressed("avoid-dynamic", 1));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn block_comment_directive_does_not_count() {
        let s = parse("/* falcon-ignore lint/suspicious/avoid-dynamic: x */\ndynamic x;\n");
        assert!(s.is_empty());
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn block_doc_comment_directive_does_not_count() {
        // `/** … */` lexes as DocComment but is a block form, so it must not
        // carry a directive.
        let s = parse("/** falcon-ignore lint/suspicious/avoid-dynamic: x */\ndynamic x;\n");
        assert!(s.is_empty());
        assert!(!s.is_suppressed("avoid-dynamic", 1));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn line_doc_comment_directive_suppresses() {
        // `///` is a line-form doc comment and does carry a directive.
        let s = parse("/// falcon-ignore lint/suspicious/avoid-dynamic: legacy\ndynamic x;\n");
        assert!(s.is_suppressed("avoid-dynamic", 1));
        assert!(s.diagnostics().is_empty());
    }

    #[test]
    fn comment_after_code_targets_its_own_line() {
        let src = "void f() {}\ndynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic: y\n";
        let s = parse(src);
        assert!(s.is_suppressed("avoid-dynamic", 1));
        assert!(!s.is_suppressed("avoid-dynamic", 2));
    }

    #[test]
    fn line_for_offset_maps_offsets() {
        let src = "aa\nbbb\nc";
        let s = parse(src);
        assert_eq!(s.line_for_offset(0), 0);
        assert_eq!(s.line_for_offset(3), 1);
        assert_eq!(s.line_for_offset(7), 2);
    }
}
