//! Falcon's lint rules.
//!
//! Rules are organized biome-style by group under `lint/<group>/<rule>.rs`
//! (complexity, correctness, performance, style, suspicious). Each rule is a
//! zero-sized struct implementing `Rule`. The metadata table supplies constructors,
//! execution identities, configuration defaults, and upstream provenance.
//! Cross-file rules implement `CrossFileRule` and live under `cross_file/`.

pub(crate) mod cross_file;
pub mod lint;
pub(crate) mod member_order;
pub mod meta;
pub mod schema;

use std::collections::HashMap;
use std::path::Path;

use falcon_analyze::{
    CrossFileRule, CrossFileRuleRegistry, FileSuppressions, PackageIdentity, ProjectFile, Rule,
    RuleRegistry, SemanticIndexes, canonical_or_lexical,
};
use falcon_config::{FalconConfig, ResolvedSeverity, Rules};
use falcon_diagnostics::{Diagnostic, Severity};

use crate::meta::{RULE_METADATA, RuleKind, meta_for, suppression_lookup};

/// Whether a registered file rule needs semantic resolution; unknown names return false.
pub fn rule_requires_resolution(name: &str) -> bool {
    meta_for(name).is_some_and(|meta| {
        matches!(
            meta.kind,
            RuleKind::File {
                needs_resolution: true,
                ..
            }
        )
    })
}

/// Whether any enabled rule in the registry needs semantic resolution.
pub fn requires_resolution(registry: &RuleRegistry) -> bool {
    registry
        .rules()
        .iter()
        .any(|(name, _)| rule_requires_resolution(name))
}

fn resolve_rules(
    config: &FalconConfig,
) -> impl Iterator<Item = (&'static str, Box<dyn Rule>)> + '_ {
    for warning in config_warnings(config) {
        eprintln!("{warning}");
    }
    RULE_METADATA.iter().filter_map(|meta| match meta.kind {
        RuleKind::File { make, .. }
            if config.is_rule_enabled_anywhere(
                meta.group.as_str(),
                meta.name,
                meta.recommended,
                meta.domain_names(),
            ) =>
        {
            Some((meta.name, make()))
        }
        _ => None,
    })
}

pub fn build_registry(config: &FalconConfig) -> RuleRegistry {
    let mut registry = RuleRegistry::with_lookup(suppression_lookup);
    for (name, rule) in resolve_rules(config) {
        registry.register(name, rule);
    }
    registry
}

pub fn build_cross_file_registry(config: &FalconConfig) -> CrossFileRuleRegistry {
    let mut registry = CrossFileRuleRegistry::new();
    for rule in resolve_cross_file_rules(config) {
        registry.register(rule);
    }
    registry
}

pub fn run_cross_file(
    registry: &CrossFileRuleRegistry,
    files: &[ProjectFile],
    config: &FalconConfig,
    indexes: Option<&SemanticIndexes>,
) -> Vec<Diagnostic> {
    let mut diagnostics = registry.run_all(files, config, indexes);
    let sources: HashMap<_, &str> = files
        .iter()
        .map(|file| (file.path.to_string_lossy(), file.source.as_str()))
        .collect();
    let mut suppressions = HashMap::new();
    diagnostics.retain(|diagnostic| {
        let Some((path, source)) = sources.get_key_value(diagnostic.file_path.as_str()) else {
            return true;
        };
        let suppression = suppressions
            .entry(path.as_ref())
            .or_insert_with(|| FileSuppressions::parse(source, path, suppression_lookup));
        !suppression.is_suppressed(
            diagnostic.rule,
            suppression.line_for_offset(diagnostic.span.start),
        )
    });
    apply_severities(&mut diagnostics, config);
    diagnostics
}

pub fn package_identities<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Vec<PackageIdentity> {
    let mut directories: Vec<_> = paths
        .into_iter()
        .filter_map(Path::parent)
        .flat_map(Path::ancestors)
        .collect();
    directories.sort_unstable();
    directories.dedup();
    let mut packages: Vec<_> = directories
        .into_iter()
        .map(|directory| directory.join("pubspec.yaml"))
        .filter(|manifest| manifest.is_file())
        .map(|manifest| package_identity(&manifest))
        .collect();
    packages.sort_unstable_by(|left, right| left.lib_root.cmp(&right.lib_root));
    packages.dedup_by(|left, right| left.lib_root == right.lib_root);
    packages
}

fn package_identity(manifest: &Path) -> PackageIdentity {
    let name = std::fs::read_to_string(manifest)
        .ok()
        .and_then(|source| package_name(&source))
        .unwrap_or_default();
    PackageIdentity {
        name,
        lib_root: canonical_or_lexical(&manifest.parent().unwrap().join("lib")),
    }
}

fn package_name(source: &str) -> Option<String> {
    match serde_yaml::from_str::<serde_yaml::Value>(source) {
        Ok(manifest) => manifest.get("name")?.as_str().map(str::to_owned),
        Err(_) => source.lines().find_map(|line| {
            let value = line.strip_prefix("name:")?.trim();
            serde_yaml::from_str::<String>(value).ok()
        }),
    }
}

/// Resolve per-path severities and discard diagnostics disabled by configuration.
pub fn apply_severities(diags: &mut Vec<Diagnostic>, config: &FalconConfig) {
    diags.retain_mut(|diag| {
        let Some(meta) = meta_for(diag.rule) else {
            return true;
        };
        let resolved = if meta.is_cross_file() {
            config.resolve_cross_file_rule_for(
                &diag.file_path,
                meta.group.as_str(),
                meta.name,
                meta.recommended,
            )
        } else {
            config.resolve_rule_for(
                &diag.file_path,
                meta.group.as_str(),
                meta.name,
                meta.recommended,
                meta.domain_names(),
            )
        };
        match resolved {
            Some(sev) => {
                diag.severity = to_severity(sev);
                true
            }
            None => false,
        }
    });
}

fn to_severity(sev: ResolvedSeverity) -> Severity {
    match sev {
        ResolvedSeverity::Info => Severity::Info,
        ResolvedSeverity::Warn => Severity::Warning,
        ResolvedSeverity::Error => Severity::Error,
    }
}

fn config_warnings(config: &FalconConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    check_rule_groups(
        &config.linter.rules,
        "falcon.json",
        Section::Linter,
        &mut warnings,
    );
    check_rule_groups(
        &config.cross_file.rules,
        "falcon.json",
        Section::CrossFile,
        &mut warnings,
    );
    for (idx, over) in config.overrides.iter().enumerate() {
        let ctx = format!("overrides[{idx}]");
        if let Some(linter) = &over.linter {
            check_rule_groups(&linter.rules, &ctx, Section::Linter, &mut warnings);
        }
        if let Some(cross_file) = &over.cross_file {
            check_rule_groups(&cross_file.rules, &ctx, Section::CrossFile, &mut warnings);
        }
    }
    warnings
}

#[derive(Clone, Copy)]
enum Section {
    Linter,
    CrossFile,
}

fn check_rule_groups(rules: &Rules, ctx: &str, section: Section, warnings: &mut Vec<String>) {
    for (group, group_rules) in &rules.groups {
        for name in group_rules.keys() {
            let Some(meta) = meta_for(name) else {
                warnings.push(format!(
                    "warning: {ctx} configures unknown rule `{name}` (under group `{group}`)"
                ));
                continue;
            };
            if meta.group.as_str() != group.as_str() {
                warnings.push(format!(
                    "warning: {ctx} configures rule `{name}` under group `{group}`, \
                     but it belongs to `{}`",
                    meta.group.as_str()
                ));
                continue;
            }
            match (section, meta.is_cross_file()) {
                (Section::Linter, true) => warnings.push(format!(
                    "warning: {ctx} configures `{name}` under `linter.rules`, but it is a \
                     cross-file rule; configure it under `cross-file.rules`"
                )),
                (Section::CrossFile, false) => warnings.push(format!(
                    "warning: {ctx} configures `{name}` under `cross-file.rules`, but it is a \
                     file-level rule; configure it under `linter.rules`"
                )),
                _ => {}
            }
        }
    }
}

pub fn all_rules() -> Vec<(&'static str, Box<dyn Rule>)> {
    RULE_METADATA
        .iter()
        .filter_map(|meta| match meta.kind {
            RuleKind::File { make, .. } => Some((meta.name, make())),
            RuleKind::CrossFile(_) => None,
        })
        .collect()
}

pub fn all_cross_file_rules() -> Vec<(&'static str, Box<dyn CrossFileRule>)> {
    RULE_METADATA
        .iter()
        .filter_map(|meta| match meta.kind {
            RuleKind::CrossFile(make) => Some((meta.name, make())),
            RuleKind::File { .. } => None,
        })
        .collect()
}

fn resolve_cross_file_rules(
    config: &FalconConfig,
) -> impl Iterator<Item = Box<dyn CrossFileRule>> + '_ {
    RULE_METADATA.iter().filter_map(|meta| match meta.kind {
        RuleKind::CrossFile(make)
            if config.is_cross_file_rule_enabled_anywhere(
                meta.group.as_str(),
                meta.name,
                meta.recommended,
            ) =>
        {
            Some(make())
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::config_warnings;
    use falcon_config::FalconConfig;
    #[test]
    fn package_identity_keeps_name_when_other_yaml_is_invalid() {
        for source in [
            "name: app\ndependencies: [\n",
            "name: 'app' # comment\ndependencies: [\n",
            "name: \"app\"\nversion: 1\nversion: 2\n",
        ] {
            assert_eq!(super::package_name(source).as_deref(), Some("app"));
        }
    }

    #[test]
    fn config_warnings_cover_base_and_overrides() {
        let config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": {
                    "style": { "totally_fake_rule": "warn" }
                }
            },
            "overrides": [
                {
                    "includes": ["a/**"],
                    "linter": { "rules": { "correctness": { "another_fake_rule": "warn" } } }
                },
                {
                    "includes": ["b/**"],
                    // `no-magic-number` is a real rule, but it belongs to `style`.
                    "linter": { "rules": { "complexity": { "no-magic-number": "warn" } } }
                }
            ]
        }))
        .expect("valid config");

        let warnings = config_warnings(&config);

        assert!(
            warnings.iter().any(
                |w| w.contains("falcon.json") && w.contains("unknown rule `totally_fake_rule`")
            ),
            "base unknown-rule warning missing: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("overrides[0]")
                    && w.contains("unknown rule `another_fake_rule`")),
            "overrides[0] unknown-rule warning missing: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("overrides[1]")
                && w.contains("`no-magic-number`")
                && w.contains("belongs to `style`")),
            "overrides[1] wrong-group warning missing: {warnings:?}"
        );
        assert_eq!(warnings.len(), 3, "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn config_warnings_flag_wrong_section() {
        let config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": { "correctness": { "unused-files": "warn" } }
            },
            "project": {
                "rules": { "suspicious": { "avoid-dynamic": "warn" } }
            }
        }))
        .expect("valid config");

        let warnings = config_warnings(&config);

        assert!(
            warnings.iter().any(|w| w.contains("`unused-files`")
                && w.contains("cross-file rule")
                && w.contains("cross-file.rules")),
            "missing cross-file-rule-under-linter warning: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("`avoid-dynamic`")
                && w.contains("file-level rule")
                && w.contains("linter.rules")),
            "missing file-rule-under-cross-file warning: {warnings:?}"
        );
        assert_eq!(warnings.len(), 2, "unexpected warnings: {warnings:?}");
    }
}
