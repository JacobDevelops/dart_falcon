//! Inline diagnostic suppression via `// ignore:` / `// ignore_for_file:`
//! comments, mirroring the Dart analyzer's syntax so existing comments carry
//! over unchanged.
//!
//! Comments are read from a real lex pass (not a naive line scan), so a
//! suppression phrase sitting inside a string literal is never mistaken for a
//! real ignore comment — the lexer tokenizes it as part of the `StringLit`.

use std::collections::{HashMap, HashSet};

use falcon_dart_parser::lexer::Lexer;
use falcon_syntax::token::TokenKind;

/// Parsed suppression directives for a single source file.
pub struct FileSuppressions {
    /// Rules suppressed everywhere in the file (`// ignore_for_file:`).
    for_file: HashSet<String>,
    /// Rules suppressed on a specific 0-based line (`// ignore:`).
    by_line: HashMap<u32, HashSet<String>>,
    /// Byte offset of the start of each 0-based line, for offset→line lookup.
    line_starts: Vec<usize>,
}

impl FileSuppressions {
    /// Lex `source` once and collect every ignore directive it contains.
    pub fn from_source(source: &str) -> Self {
        let line_starts = compute_line_starts(source);
        let mut for_file: HashSet<String> = HashSet::new();
        let mut by_line: HashMap<u32, HashSet<String>> = HashMap::new();

        // Line of the most recent non-trivia token; lets us tell a trailing
        // `// ignore:` (code precedes it on the same line → suppress that line)
        // from a standalone one (nothing before it → suppress the next line).
        let mut last_code_line: Option<u32> = None;

        for token in Lexer::new(source).tokenize() {
            if token.is_trivia() {
                // Only `//`-style comments carry ignore directives; block
                // comments (`/* */`) are excluded, matching the analyzer.
                if matches!(token.kind, TokenKind::LineComment | TokenKind::DocComment) {
                    let text = token.text(source);
                    let line = line_for_offset(&line_starts, token.offset);
                    if let Some(directive) = parse_ignore_comment(text) {
                        match directive {
                            Directive::ForFile(rules) => for_file.extend(rules),
                            Directive::Line(rules) => {
                                let target = if last_code_line == Some(line) {
                                    line
                                } else {
                                    line + 1
                                };
                                by_line.entry(target).or_default().extend(rules);
                            }
                        }
                    }
                }
                continue;
            }
            last_code_line = Some(line_for_offset(&line_starts, token.offset));
        }

        Self {
            for_file,
            by_line,
            line_starts,
        }
    }

    /// The 0-based line a byte offset falls on (used for a diagnostic's span
    /// start). Shared here so callers don't recompute a line table.
    pub fn line_for_offset(&self, offset: usize) -> u32 {
        line_for_offset(&self.line_starts, offset)
    }

    /// Whether `rule` is suppressed on the given 0-based `line`.
    pub fn is_suppressed(&self, rule: &str, line: u32) -> bool {
        self.for_file.contains(rule)
            || self
                .by_line
                .get(&line)
                .is_some_and(|rules| rules.contains(rule))
    }

    /// True when the file carries no directives at all (skip filtering).
    pub fn is_empty(&self) -> bool {
        self.for_file.is_empty() && self.by_line.is_empty()
    }
}

enum Directive {
    ForFile(Vec<String>),
    Line(Vec<String>),
}

/// Parse a `//`-comment's text into an ignore directive, if it is one.
///
/// Accepts `//`+ then optional spaces then `ignore:` / `ignore_for_file:` then
/// a comma-separated rule list — matching what the Dart analyzer recognizes,
/// including the no-space `//ignore:` form.
fn parse_ignore_comment(text: &str) -> Option<Directive> {
    let body = text.trim_start_matches('/').trim_start();
    if let Some(rest) = body.strip_prefix("ignore_for_file:") {
        Some(Directive::ForFile(parse_rule_list(rest)))
    } else {
        body.strip_prefix("ignore:")
            .map(|rest| Directive::Line(parse_rule_list(rest)))
    }
}

/// Split a comma-separated rule list, trimming whitespace and dropping empties.
/// Names are kept verbatim and matched exactly against registered rule names.
fn parse_rule_list(rest: &str) -> Vec<String> {
    rest.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
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

    fn rules_on(source: &str, line: u32) -> Vec<String> {
        let s = FileSuppressions::from_source(source);
        let mut got: Vec<String> = s
            .by_line
            .get(&line)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        got.sort();
        got
    }

    #[test]
    fn same_line_suppression() {
        // `dynamic x;  // ignore: avoid-dynamic` — code precedes the comment,
        // so it suppresses this same line (line 0).
        let s = FileSuppressions::from_source("dynamic x; // ignore: avoid-dynamic\n");
        assert!(s.is_suppressed("avoid-dynamic", 0));
        assert!(!s.is_suppressed("avoid-dynamic", 1));
    }

    #[test]
    fn next_line_suppression() {
        // Comment alone on line 0 → suppresses line 1.
        let s = FileSuppressions::from_source("// ignore: avoid-dynamic\ndynamic x;\n");
        assert!(s.is_suppressed("avoid-dynamic", 1));
        assert!(!s.is_suppressed("avoid-dynamic", 0));
    }

    #[test]
    fn multiple_rules() {
        assert_eq!(
            rules_on("// ignore: rule-a, rule-b , rule-c\n", 1),
            vec!["rule-a", "rule-b", "rule-c"]
        );
    }

    #[test]
    fn ignore_for_file_applies_anywhere() {
        let s = FileSuppressions::from_source(
            "// ignore_for_file: avoid-dynamic\nvoid f() {}\ndynamic y;\n",
        );
        assert!(s.is_suppressed("avoid-dynamic", 0));
        assert!(s.is_suppressed("avoid-dynamic", 2));
        assert!(s.is_suppressed("avoid-dynamic", 999));
    }

    #[test]
    fn unknown_rule_names_are_harmless() {
        let s = FileSuppressions::from_source("// ignore: not-a-real-rule\ndynamic x;\n");
        assert!(s.is_suppressed("not-a-real-rule", 1));
        assert!(!s.is_suppressed("avoid-dynamic", 1));
    }

    #[test]
    fn no_space_ignore_is_accepted() {
        let s = FileSuppressions::from_source("//ignore: avoid-dynamic\ndynamic x;\n");
        assert!(s.is_suppressed("avoid-dynamic", 1));
    }

    #[test]
    fn comment_after_code_targets_its_own_line() {
        let src = "void f() {}\ndynamic x = 1; // ignore: avoid-dynamic\n";
        let s = FileSuppressions::from_source(src);
        assert!(s.is_suppressed("avoid-dynamic", 1));
        assert!(!s.is_suppressed("avoid-dynamic", 2));
    }

    #[test]
    fn suppression_inside_string_literal_does_not_count() {
        // The `// ignore:` sits inside a string, so the lexer never sees it as
        // a comment and it must not suppress anything.
        let src = "var s = '// ignore: avoid-dynamic';\ndynamic x;\n";
        let s = FileSuppressions::from_source(src);
        assert!(s.is_empty(), "string-literal ignore must not register");
        assert!(!s.is_suppressed("avoid-dynamic", 1));
    }

    #[test]
    fn doc_comment_triple_slash_is_accepted() {
        // The analyzer's `//+` allows extra slashes; `/// ignore:` still counts.
        let s = FileSuppressions::from_source("/// ignore: avoid-dynamic\ndynamic x;\n");
        assert!(s.is_suppressed("avoid-dynamic", 1));
    }

    #[test]
    fn block_comment_ignore_does_not_count() {
        let s = FileSuppressions::from_source("/* ignore: avoid-dynamic */\ndynamic x;\n");
        assert!(s.is_empty());
    }

    #[test]
    fn line_for_offset_maps_offsets() {
        let src = "aa\nbbb\nc";
        let s = FileSuppressions::from_source(src);
        assert_eq!(s.line_for_offset(0), 0); // 'a'
        assert_eq!(s.line_for_offset(3), 1); // 'b'
        assert_eq!(s.line_for_offset(7), 2); // 'c'
    }
}
