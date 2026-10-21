//! Report imports forbidden by package-relative architectural boundaries.
//!
//! Configure `rules: [{files: ["lib/domain/**"], deny: ["package:flutter/**"],
//! message: "Keep domain code independent of Flutter."}]` in this rule's options.
//! Both lists are required, nonempty lists of nonempty glob strings; `message`
//! is required and nonblank. An invalid entry (including any invalid glob or
//! non-string list member) is ignored, and no configuration means no reports.
//! Globs are case-sensitive, match the entire string, and use `/` separators:
//! `*` does not cross directories, while `**` does. Source `files` paths are
//! relative to their nearest enclosing pubspec directory (including `lib/`).
//! Files without an enclosing pubspec are skipped.
//!
//! Each `deny` pattern matches the decoded import URI, without quotes, and the
//! normalized package-relative target of relative or same-package imports.
//! Local target matching does not require the target to exist. External package
//! and SDK imports match only their URI. Primary and all conditional URIs are
//! checked, including inactive alternatives; prefixes, deferred loading and
//! combinators do not exempt an import. Each matching entry reports its custom
//! message once per URI, even if several of its patterns match. Overlapping
//! entries can therefore produce multiple reports. Exports and parts are not
//! imports and are deliberately outside this rule's boundary policy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, PackageIdentity, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use glob::{MatchOptions, Pattern};
use serde_json::Value;

use crate::lint::options;

use super::{ResolvedRef, canonical_or_lexical, enclosing_pubspec, resolve_directive_uri};

#[derive(Default)]
pub struct BannedImports;

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

struct Boundary<'a> {
    files: Vec<Pattern>,
    deny: Vec<Pattern>,
    message: &'a str,
}

impl CrossFileRule for BannedImports {
    fn prepare_project(
        &self,
        files: &[ProjectFile],
        config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut compiled: HashMap<*const Value, Vec<Boundary<'_>>> = HashMap::new();
        let mut packages: HashMap<PathBuf, PackageIdentity> = HashMap::new();
        let mut diagnostics = Vec::new();
        for file in files {
            let file_path = file.path.to_string_lossy();
            let Some(settings) = options::cross_file_for_rule(config, &file_path, BannedImports::NAME) else {
                continue;
            };
            let boundaries = compiled
                .entry(std::ptr::from_ref(settings))
                .or_insert_with(|| parse_boundaries(settings));
            if boundaries.is_empty() {
                continue;
            }
            let Some(manifest) = enclosing_pubspec(&file.path) else {
                continue;
            };
            let package = packages
                .entry(manifest.clone())
                .or_insert_with(|| super::package_identity(&manifest));
            let root = manifest.parent().unwrap();
            let Some(relative) = package_relative(&file.path, root, package) else {
                continue;
            };
            let mut applicable = boundaries
                .iter()
                .filter(|entry| matches(&entry.files, &relative))
                .peekable();
            if applicable.peek().is_none() {
                continue;
            }
            for import in file.program().imports() {
                let uris = std::iter::once(import.uri()).chain(
                    import
                        .configurable_uris()
                        .map(|alternative| alternative.uri()),
                );
                for uri in uris {
                    let value = uri.value();
                    let target = if !value.contains(':') || value.starts_with("package:") {
                        match resolve_directive_uri(&file.path, &value, Some(package)) {
                            Some(ResolvedRef::Path(path)) => package_relative(&path, root, package),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    for entry in applicable.clone() {
                        if matches(&entry.deny, &value)
                            || target
                                .as_deref()
                                .is_some_and(|path| matches(&entry.deny, path))
                        {
                            diagnostics.push(Diagnostic::new(
                                "",
                                Severity::Warning,
                                entry.message,
                                file_path.as_ref(),
                                uri.span(),
                            ));
                        }
                    }
                }
            }
        }
        diagnostics
    }
}

fn parse_boundaries(settings: &Value) -> Vec<Boundary<'_>> {
    let Some(entries) = settings.get("rules").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let message = entry.get("message")?.as_str()?;
            if message.trim().is_empty() {
                return None;
            }
            Some(Boundary {
                files: patterns(entry, "files")?,
                deny: patterns(entry, "deny")?,
                message,
            })
        })
        .collect()
}

fn patterns(entry: &Value, key: &str) -> Option<Vec<Pattern>> {
    let values = entry.get(key)?.as_array()?;
    if values.is_empty() || values.iter().any(|value| !value.is_string()) {
        return None;
    }
    options::strings(Some(entry), key)?
        .map(|value| {
            if value.is_empty() {
                None
            } else {
                Pattern::new(value).ok()
            }
        })
        .collect()
}

fn matches(patterns: &[Pattern], value: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern.matches_with(value, MATCH_OPTIONS))
}

fn package_relative(path: &Path, root: &Path, package: &PackageIdentity) -> Option<String> {
    let path = canonical_or_lexical(path);
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }
    // A package's lib directory can be symlinked outside its pubspec directory.
    let relative = path.strip_prefix(&package.lib_root).ok()?;
    Some(format!(
        "lib/{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn malformed_entries_do_not_broaden_a_boundary() {
        for entry in [
            json!({"deny": ["*"], "message": "blocked"}),
            json!({"files": [], "deny": ["*"], "message": "blocked"}),
            json!({"files": ["**", null], "deny": ["*"], "message": "blocked"}),
            json!({"files": ["**"], "deny": ["*", false], "message": "blocked"}),
            json!({"files": ["["], "deny": ["*"], "message": "blocked"}),
            json!({"files": ["**"], "deny": ["["], "message": "blocked"}),
            json!({"files": ["**"], "deny": [""], "message": "blocked"}),
            json!({"files": ["**"], "deny": ["*"], "message": " "}),
        ] {
            assert!(parse_boundaries(&json!({"rules": [entry]})).is_empty());
        }
    }

    #[test]
    fn globs_respect_case_and_directory_boundaries() {
        let settings = json!({"rules": [{
            "files": ["lib/domain/**"],
            "deny": ["package:legacy/*.dart"],
            "message": "Use the supported API."
        }]});
        let entries = parse_boundaries(&settings);
        assert_eq!(entries.len(), 1);
        assert!(matches(&entries[0].files, "lib/domain/nested/model.dart"));
        assert!(!matches(&entries[0].files, "lib/Domain/model.dart"));
        assert!(!matches(&entries[0].files, "lib/domain_extra/model.dart"));
        assert!(matches(&entries[0].deny, "package:legacy/client.dart"));
        assert!(!matches(
            &entries[0].deny,
            "package:legacy/nested/client.dart"
        ));
    }
}
