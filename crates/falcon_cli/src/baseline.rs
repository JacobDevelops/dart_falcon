//! Versioned, occurrence-counted baselines for existing lint findings.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use falcon_diagnostics::{Diagnostic, RESERVED_DIAGNOSTIC_IDS};
use serde::{Deserialize, Serialize};

const VERSION: u32 = 1;

/// A baseline stores counts rather than a set so new duplicate findings remain visible.
#[derive(Debug, Serialize)]
pub struct Baseline {
    version: u32,
    entries: BTreeMap<String, usize>,
}

/// Counts refer to occurrences, not distinct fingerprints.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BaselineStats {
    pub suppressed: usize,
    pub stale: usize,
}

impl Baseline {
    /// Capture lint findings; missing sources and invalid source spans are errors.
    pub fn from_diagnostics<'a>(
        diagnostics: &[Diagnostic],
        root: &Path,
        mut source: impl FnMut(&str) -> Option<&'a str>,
    ) -> Result<Self, String> {
        let mut entries = BTreeMap::new();
        for diagnostic in diagnostics.iter().filter(|diagnostic| is_lint(diagnostic)) {
            let key = checked_fingerprint(diagnostic, root, &mut source)?;
            *entries.entry(key).or_insert(0) += 1;
        }
        Ok(Self {
            version: VERSION,
            entries,
        })
    }

    /// Load a baseline, rejecting unknown versions, fields, and invalid counts or hashes.
    pub fn load(path: &Path) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct BaselineFile {
            version: u32,
            entries: BTreeMap<String, usize>,
        }

        let bytes = std::fs::read(path)
            .map_err(|error| format!("Cannot read baseline {}: {error}", path.display()))?;
        let baseline: BaselineFile = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Invalid baseline {}: {error}", path.display()))?;
        if baseline.version != VERSION {
            return Err(format!(
                "Unsupported baseline version {} in {}",
                baseline.version,
                path.display()
            ));
        }
        let mut total = 0usize;
        for (key, count) in &baseline.entries {
            if key.len() != 32
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(format!(
                    "Invalid baseline fingerprint in {}",
                    path.display()
                ));
            }
            if *count == 0 {
                return Err(format!(
                    "Baseline occurrence counts must be positive in {}",
                    path.display()
                ));
            }
            total = total.checked_add(*count).ok_or_else(|| {
                format!("Baseline occurrence count overflow in {}", path.display())
            })?;
        }
        Ok(Self {
            version: baseline.version,
            entries: baseline.entries,
        })
    }

    /// Write deterministic JSON with fingerprints in sorted order.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Cannot serialize baseline: {error}"))?;
        bytes.push(b'\n');
        std::fs::write(path, bytes)
            .map_err(|error| format!("Cannot write baseline {}: {error}", path.display()))
    }

    /// Remove only recorded lint occurrences, preserving diagnostic order.
    ///
    /// Operational and syntax diagnostics are never suppressed or counted as stale.
    /// Source lookup errors leave the input unchanged. Applying does not modify the baseline.
    pub fn apply<'a>(
        &self,
        diagnostics: &mut Vec<Diagnostic>,
        root: &Path,
        mut source: impl FnMut(&str) -> Option<&'a str>,
    ) -> Result<BaselineStats, String> {
        let mut remaining: BTreeMap<_, _> = self
            .entries
            .iter()
            .map(|(key, &count)| (key.as_str(), count))
            .collect();
        let mut suppress = Vec::with_capacity(diagnostics.len());
        let mut stats = BaselineStats::default();
        for diagnostic in diagnostics.iter() {
            let count = if is_lint(diagnostic) {
                let key = checked_fingerprint(diagnostic, root, &mut source)?;
                remaining.get_mut(key.as_str())
            } else {
                None
            };
            let matched = count.is_some_and(|count| {
                if *count == 0 {
                    return false;
                }
                *count -= 1;
                true
            });
            stats.suppressed += usize::from(matched);
            suppress.push(matched);
        }
        stats.stale = remaining.values().sum();
        let mut suppress = suppress.into_iter();
        diagnostics.retain(|_| !suppress.next().expect("one decision per diagnostic"));
        Ok(stats)
    }
}

fn is_lint(diagnostic: &Diagnostic) -> bool {
    !RESERVED_DIAGNOSTIC_IDS.contains(&diagnostic.rule)
}

fn checked_fingerprint<'a>(
    diagnostic: &Diagnostic,
    root: &Path,
    source: &mut impl FnMut(&str) -> Option<&'a str>,
) -> Result<String, String> {
    let text = source(&diagnostic.file_path).ok_or_else(|| {
        format!(
            "Missing source for baseline finding in {}",
            diagnostic.file_path
        )
    })?;
    if flagged_snippet(diagnostic, text).is_none() {
        return Err(format!(
            "Invalid baseline finding span in {}",
            diagnostic.file_path
        ));
    }
    let path = try_relative_path(&diagnostic.file_path, root)?;
    Ok(fingerprint_with_path(diagnostic, text, &path))
}

/// Stable FNV-1a-128 over the rule, root-relative path, and flagged snippet.
///
/// Snippet whitespace runs become one ASCII space and edge whitespace is removed.
/// Offsets, resolved positions, severity, and message are deliberately not hashed.
/// An invalid span hashes an empty snippet; baseline capture/application reject such spans.
///
/// # Panics
/// Panics if absolute path resolution fails, for example when a relative path's cwd is unavailable.
pub fn fingerprint(diagnostic: &Diagnostic, source: &str, root: &Path) -> String {
    fingerprint_with_path(
        diagnostic,
        source,
        &relative_path(&diagnostic.file_path, root),
    )
}

fn fingerprint_with_path(diagnostic: &Diagnostic, source: &str, path: &str) -> String {
    let mut hash = 0x6c62272e07bb014262b821756295c58du128;
    let mut feed = |bytes: &[u8]| {
        for &byte in bytes {
            hash ^= u128::from(byte);
            hash = hash.wrapping_mul(0x0000000001000000000000000000013b);
        }
    };
    feed(diagnostic.rule.as_bytes());
    feed(&[0]);
    feed(path.as_bytes());
    feed(&[0]);
    let snippet = flagged_snippet(diagnostic, source).unwrap_or("");
    for (index, word) in snippet.split_whitespace().enumerate() {
        if index != 0 {
            feed(b" ");
        }
        feed(word.as_bytes());
    }
    format!("{hash:032x}")
}

fn flagged_snippet<'a>(diagnostic: &Diagnostic, source: &'a str) -> Option<&'a str> {
    let span = diagnostic.span;
    // The EOF newline rule highlights one virtual byte past a missing final newline.
    if diagnostic.rule == "eol-at-end-of-file"
        && span.start == source.len()
        && span.end.checked_sub(1) == Some(source.len())
    {
        Some("")
    } else {
        source.get(span.start..span.end)
    }
}

/// Resolve cwd-relative paths and canonicalize existing files and roots.
/// Unresolvable paths use absolute lexical normalization.
///
/// # Panics
/// Panics if absolute path resolution fails, for example when a relative path's cwd is unavailable.
/// Baseline capture and application return that error instead.
pub(crate) fn relative_path(path: &str, root: &Path) -> String {
    try_relative_path(path, root).expect("cannot resolve diagnostic path")
}

fn resolve_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("Cannot resolve baseline path {}: {error}", path.display()))?;
    match absolute.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => Ok(normalize_path(&absolute)),
    }
}

fn try_relative_path(path: &str, root: &Path) -> Result<String, String> {
    let path = resolve_path(Path::new(path))?;
    let root = resolve_path(root)?;
    let path_parts: Vec<_> = path.components().collect();
    let root_parts: Vec<_> = root.components().collect();
    let common = path_parts
        .iter()
        .zip(&root_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..root_parts.len() {
        relative.push("..");
    }
    for component in &path_parts[common..] {
        relative.push(component.as_os_str());
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some_and(|name| name != "..") {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push("..");
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_diagnostics::{Severity, Span};

    fn finding(source: &str, snippet: &str) -> Diagnostic {
        let start = source.find(snippet).unwrap();
        Diagnostic::new(
            "avoid-print",
            Severity::Warning,
            "Avoid print.",
            "/project/lib/main.dart",
            Span {
                start,
                end: start + snippet.len(),
            },
        )
    }

    #[test]
    fn fingerprint_survives_line_message_and_checkout_changes() {
        let before = "print(1);";
        let after = "// added a line\n\nprint(1);";
        let first = finding(before, before);
        let mut shifted = finding(after, before);
        shifted.message = "A revised message.".into();
        shifted.file_path = "/other/lib/main.dart".into();
        assert_eq!(
            fingerprint(&first, before, Path::new("/project")),
            fingerprint(&shifted, after, Path::new("/other")),
        );
        shifted.rule = "another-rule";
        assert_ne!(
            fingerprint(&first, before, Path::new("/project")),
            fingerprint(&shifted, after, Path::new("/other")),
        );
    }

    #[test]
    fn fingerprint_normalizes_snippet_whitespace() {
        let before = "print( 1 );";
        let after = "print(\n  1\t );";
        assert_eq!(
            fingerprint(&finding(before, before), before, Path::new("/project")),
            fingerprint(&finding(after, after), after, Path::new("/project")),
        );
    }

    #[test]
    fn duplicate_occurrences_only_suppress_recorded_count() {
        let source = "print(1);";
        let diagnostic = finding(source, source);
        let baseline = Baseline::from_diagnostics(
            std::slice::from_ref(&diagnostic),
            Path::new("/project"),
            |_| Some(source),
        )
        .unwrap();
        let mut diagnostics = vec![diagnostic.clone(), diagnostic];
        let stats = baseline
            .apply(&mut diagnostics, Path::new("/project"), |_| Some(source))
            .unwrap();
        assert_eq!(
            stats,
            BaselineStats {
                suppressed: 1,
                stale: 0
            }
        );
        assert_eq!(diagnostics.len(), 1);
        // Applying a baseline does not consume its allowances permanently.
        assert_eq!(
            baseline
                .apply(&mut diagnostics, Path::new("/project"), |_| Some(source))
                .unwrap()
                .suppressed,
            1
        );
    }

    #[test]
    fn stale_counts_include_each_missing_occurrence() {
        let source = "print(1);";
        let diagnostic = finding(source, source);
        let baseline = Baseline::from_diagnostics(
            &[diagnostic.clone(), diagnostic.clone()],
            Path::new("/project"),
            |_| Some(source),
        )
        .unwrap();
        let mut diagnostics = vec![diagnostic];
        assert_eq!(
            baseline
                .apply(&mut diagnostics, Path::new("/project"), |_| Some(source))
                .unwrap(),
            BaselineStats {
                suppressed: 1,
                stale: 1
            },
        );
        assert_eq!(
            baseline
                .apply(&mut diagnostics, Path::new("/project"), |_| None)
                .unwrap()
                .stale,
            2
        );
    }

    #[test]
    fn operational_diagnostics_are_never_suppressed() {
        let source = "print(1);";
        let diagnostics: Vec<_> = falcon_diagnostics::RESERVED_DIAGNOSTIC_IDS
            .iter()
            .map(|&rule| {
                let mut diagnostic = finding(source, source);
                diagnostic.rule = rule;
                diagnostic
            })
            .collect();
        let baseline =
            Baseline::from_diagnostics(&diagnostics, Path::new("/project"), |_| None).unwrap();
        let mut remaining = diagnostics.clone();
        assert_eq!(
            baseline
                .apply(&mut remaining, Path::new("/project"), |_| None)
                .unwrap(),
            BaselineStats::default()
        );
        assert_eq!(remaining.len(), diagnostics.len());
    }

    #[test]
    fn malformed_baselines_are_errors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("baseline.json");
        for json in [
            "not json",
            "{}",
            r#"{"version":2,"entries":{}}"#,
            r#"{"version":1,"entries":{"bad":1}}"#,
            r#"{"version":1,"entries":{"00000000000000000000000000000000":0}}"#,
            r#"{"version":1,"entries":{},"unknown":true}"#,
        ] {
            std::fs::write(&path, json).unwrap();
            assert!(Baseline::load(&path).is_err(), "accepted {json}");
        }
        assert!(Baseline::load(&directory.path().join("missing")).is_err());
    }

    #[test]
    fn baseline_file_roundtrips_deterministically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("baseline.json");
        let source = "print(1);";
        let diagnostic = finding(source, source);
        let baseline = Baseline::from_diagnostics(
            std::slice::from_ref(&diagnostic),
            Path::new("/project"),
            |_| Some(source),
        )
        .unwrap();
        baseline.write(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let loaded = Baseline::load(&path).unwrap();
        loaded.write(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let mut diagnostics = vec![diagnostic];
        assert_eq!(
            loaded
                .apply(&mut diagnostics, Path::new("/project"), |_| Some(source))
                .unwrap()
                .suppressed,
            1
        );
    }

    #[test]
    fn invalid_sources_error_without_mutating_diagnostics() {
        let source = "print(1);";
        let diagnostic = finding(source, source);
        let baseline = Baseline::from_diagnostics(
            std::slice::from_ref(&diagnostic),
            Path::new("/project"),
            |_| Some(source),
        )
        .unwrap();
        let mut diagnostics = vec![diagnostic];
        assert!(
            baseline
                .apply(&mut diagnostics, Path::new("/project"), |_| None)
                .is_err()
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(
            Baseline::from_diagnostics(&diagnostics, Path::new("/project"), |_| Some("")).is_err()
        );
    }

    #[test]
    fn virtual_eof_span_can_be_baselined() {
        let source = "print(1);";
        let mut diagnostic = finding(source, source);
        diagnostic.rule = "eol-at-end-of-file";
        diagnostic.span = Span {
            start: source.len(),
            end: source.len() + 1,
        };
        let baseline = Baseline::from_diagnostics(
            std::slice::from_ref(&diagnostic),
            Path::new("/project"),
            |_| Some(source),
        )
        .unwrap();
        let mut diagnostics = vec![diagnostic.clone()];
        assert_eq!(
            baseline
                .apply(&mut diagnostics, Path::new("/project"), |_| Some(source))
                .unwrap()
                .suppressed,
            1
        );
        diagnostic.span.end += 1;
        assert!(
            Baseline::from_diagnostics(&[diagnostic], Path::new("/project"), |_| Some(source))
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_checkout_paths_match_canonical_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let source = "print(1);";
        std::fs::write(root.join("lib/main.dart"), source).unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let canonical_root = root.canonicalize().unwrap();
        let mut diagnostic = finding(source, source);
        diagnostic.file_path = alias.join("lib/main.dart").to_str().unwrap().into();
        let through_alias = fingerprint(&diagnostic, source, &canonical_root);
        assert_eq!(
            relative_path(&diagnostic.file_path, &canonical_root),
            "lib/main.dart"
        );
        diagnostic.file_path = canonical_root
            .join("lib/main.dart")
            .to_str()
            .unwrap()
            .into();
        assert_eq!(through_alias, fingerprint(&diagnostic, source, &alias));
    }

    #[test]
    fn relative_paths_are_lexical_and_portable() {
        assert_eq!(
            relative_path("/project/lib/../lib/main.dart", Path::new("/project")),
            "lib/main.dart"
        );
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            relative_path("project/lib/main.dart", &cwd.join("project")),
            "lib/main.dart"
        );
        assert_eq!(
            relative_path("project/lib/main.dart", Path::new("project")),
            "lib/main.dart"
        );
        assert_eq!(
            relative_path("/shared/main.dart", Path::new("/project")),
            "../shared/main.dart"
        );
    }
}
