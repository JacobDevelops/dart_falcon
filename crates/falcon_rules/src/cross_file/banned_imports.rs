//! Report imports forbidden by package-relative architectural boundaries.
//!
//! Configure `rules: [{files: ["lib/domain/**"], deny: ["package:flutter/**"],
//! message: "Keep domain code independent of Flutter."}]` in this rule's options.
//! Both lists are required, nonempty lists of nonempty glob strings; `message`
//! is required and nonblank. Invalid entries are rejected during configuration
//! validation; no configuration means no reports.
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
use falcon_diagnostics::{CONFIG_DIAGNOSTIC_ID, Diagnostic, Severity};
use falcon_syntax::cst::Span;
use glob::MatchOptions;

use crate::lint::options::{self, Glob, nonblank_message, nonempty_patterns};

use super::{ResolvedRef, canonical_or_lexical, enclosing_pubspec, resolve_directive_uri};

#[derive(Default)]
pub struct BannedImports;

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Package-relative import boundaries applied to matching source files.
    rules: Vec<Boundary>,
}

crate::lint::options::impl_options!(Options);

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Boundary {
    /// Source-file globs selecting where this boundary applies.
    #[serde(deserialize_with = "nonempty_patterns")]
    #[schemars(length(min = 1))]
    files: Vec<Glob>,
    /// Import URI or package-relative target globs prohibited within `files`.
    #[serde(deserialize_with = "nonempty_patterns")]
    #[schemars(length(min = 1))]
    deny: Vec<Glob>,
    /// Warning text emitted when a prohibited import matches.
    #[serde(deserialize_with = "nonblank_message")]
    message: String,
}

impl CrossFileRule for BannedImports {
    fn prepare_project(
        &self,
        files: &[ProjectFile],
        config: &FalconConfig,
        _indexes: Option<&falcon_analyze::SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let mut packages: HashMap<PathBuf, PackageIdentity> = HashMap::new();
        let mut diagnostics = Vec::new();
        for file in files {
            let file_path = file.path.to_string_lossy();
            let boundaries = match options::cross_file::<Options>(config, &file_path, Self::NAME) {
                Ok(options) => &options.rules,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(
                        CONFIG_DIAGNOSTIC_ID,
                        Severity::Error,
                        error.to_string(),
                        file_path.as_ref(),
                        Span::new(0, 0),
                    ));
                    continue;
                }
            };
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
                                &entry.message,
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

fn matches(patterns: &[Glob], value: &str) -> bool {
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
    fn globs_respect_case_and_directory_boundaries() {
        let settings = json!({"rules": [{
            "files": ["lib/domain/**"],
            "deny": ["package:legacy/*.dart"],
            "message": "Use the supported API."
        }]});
        let options: Options = serde_json::from_value(settings).unwrap();
        let entries = options.rules;
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
