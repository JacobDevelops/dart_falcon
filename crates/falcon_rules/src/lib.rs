//! Falcon's lint rules.
//!
//! Rules are organized biome-style by group under `lint/<group>/<rule>.rs`
//! (a11y, complexity, correctness, performance, security, style, suspicious). Each rule is a
//! zero-sized struct implementing `Rule`. The metadata table supplies constructors,
//! execution identities, configuration defaults, and upstream provenance.
//! Cross-file rules implement `CrossFileRule` and live under `cross_file/`.

pub(crate) mod cross_file;
pub mod lint;
pub(crate) mod member_order;
pub mod meta;
pub mod schema;

pub(crate) use meta::declare_rules;

#[cfg(test)]
#[path = "../tests/support/inference.rs"]
mod inference;

use std::collections::HashMap;
use std::path::Path;

use falcon_analyze::{
    CrossFileRule, CrossFileRuleRegistry, FileSuppressions, PackageIdentity, ProjectFile, Rule,
    RuleRegistry, SemanticIndexes, canonical_or_lexical,
};
use falcon_config::{ConfigError, FalconConfig, LoadedConfig, ResolvedSeverity, Rules};
use falcon_diagnostics::{Diagnostic, Severity};

use crate::meta::{RULE_METADATA, RuleKind, meta_for, suppression_lookup};

/// Load configuration, canonicalize legacy rule ids, and return all warnings.
///
/// # Errors
///
/// Returns an error when the configuration cannot be read or is invalid.
pub fn load_config(path: &Path) -> Result<LoadedConfig, ConfigError> {
    falcon_config::deserialize_config(path).and_then(canonicalize_loaded_config)
}

/// Load a discovered configuration or defaults through the canonicalizing loader.
///
/// # Errors
///
/// Returns an error when a discovered configuration cannot be loaded.
pub fn load_or_default(start_dir: &Path) -> Result<LoadedConfig, ConfigError> {
    match falcon_config::find_config(start_dir) {
        Some(path) => {
            let mut loaded = load_config(&path)?;
            if falcon_config::is_home_config(&path) {
                loaded.config.resolve_paths(start_dir);
            }
            Ok(loaded)
        }
        None => {
            let mut config = FalconConfig::default();
            config.resolve_paths(start_dir);
            canonicalize_loaded_config(LoadedConfig {
                config,
                warnings: Vec::new(),
            })
        }
    }
}

/// Canonicalize aliases and validate registered rule names and typed options.
///
/// # Errors
///
/// Returns an error for unknown rules, unsupported sections, or invalid options.
pub fn canonicalize_loaded_config(mut loaded: LoadedConfig) -> Result<LoadedConfig, ConfigError> {
    validate_rule_names(&loaded.config.linter.rules, "linter.rules", Section::Linter)?;
    validate_rule_names(
        &loaded.config.cross_file.rules,
        "cross-file.rules",
        Section::CrossFile,
    )?;
    for (index, entry) in loaded.config.overrides.iter().enumerate() {
        if let Some(linter) = &entry.linter {
            validate_rule_names(
                &linter.rules,
                &format!("overrides[{index}].linter.rules"),
                Section::Linter,
            )?;
        }
        if let Some(cross_file) = &entry.cross_file {
            validate_rule_names(
                &cross_file.rules,
                &format!("overrides[{index}].cross-file.rules"),
                Section::CrossFile,
            )?;
        }
    }
    loaded
        .warnings
        .extend(meta::canonicalize_config(&mut loaded.config));
    loaded.warnings.extend(config_warnings(&loaded.config));
    validate_config_options(&loaded.config)?;
    Ok(loaded)
}

fn validate_rule_names(rules: &Rules, path: &str, section: Section) -> Result<(), ConfigError> {
    for (group, entries) in &rules.groups {
        for name in entries.keys() {
            let Some(metadata) = meta_for(name) else {
                return Err(ConfigError(format!(
                    "unknown rule key `{path}.{group}.{name}`"
                )));
            };
            if metadata.group.as_str() != group
                || metadata.is_cross_file() != matches!(section, Section::CrossFile)
            {
                let expected_section = if metadata.is_cross_file() {
                    "cross-file"
                } else {
                    "linter"
                };
                return Err(ConfigError(format!(
                    "unknown rule key `{path}.{group}.{name}`; configure it under \
                     `{expected_section}.rules.{}.{}`",
                    metadata.group.as_str(),
                    metadata.name
                )));
            }
        }
    }
    Ok(())
}

/// Rewrite legacy snake_case option keys recursively, preserving canonical keys.
pub fn canonicalize_option_keys(options: &mut serde_json::Value, path: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    match options {
        serde_json::Value::Object(entries) => {
            let aliases: Vec<_> = entries
                .keys()
                .filter(|key| key.contains('_'))
                .cloned()
                .collect();
            for alias in aliases {
                let canonical = alias.replace('_', "-");
                let value = entries.shift_remove(&alias).expect("existing option key");
                warnings.push(format!(
                    "deprecated option key `{path}.{alias}`; use `{canonical}` — \
                     run `falcon migrate` to update"
                ));
                entries.entry(canonical).or_insert(value);
            }
            for (key, value) in entries {
                warnings.extend(canonicalize_option_keys(value, &format!("{path}.{key}")));
            }
        }
        serde_json::Value::Array(entries) => {
            for (index, value) in entries.iter_mut().enumerate() {
                warnings.extend(canonicalize_option_keys(value, &format!("{path}[{index}]")));
            }
        }
        _ => {}
    }
    warnings
}

fn validate_config_options(config: &FalconConfig) -> Result<(), ConfigError> {
    let validate = |rules: &Rules, path: &str| {
        for (group, entries) in &rules.groups {
            for (name, configuration) in entries {
                if let Some(options) = configuration.options_data() {
                    lint::options::validate_rule(
                        name,
                        options,
                        &format!("{path}.{group}.{name}.options"),
                    )?;
                }
            }
        }
        Ok(())
    };
    validate(&config.linter.rules, "linter.rules")?;
    validate(&config.cross_file.rules, "cross-file.rules")?;
    for (index, entry) in config.overrides.iter().enumerate() {
        if let Some(section) = &entry.linter {
            validate(&section.rules, &format!("overrides[{index}].linter.rules"))?;
        }
        if let Some(section) = &entry.cross_file {
            validate(
                &section.rules,
                &format!("overrides[{index}].cross-file.rules"),
            )?;
        }
    }
    Ok(())
}

/// Whether a registered rule needs semantic resolution; unknown names return false.
pub fn rule_requires_resolution(name: &str) -> bool {
    meta_for(name).is_some_and(|meta| {
        matches!(
            meta.kind,
            RuleKind::File {
                needs_resolution: true,
                ..
            } | RuleKind::CrossFile {
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

/// Whether an enabled cross-file rule needs semantic resolution.
pub fn cross_file_requires_resolution(config: &FalconConfig) -> bool {
    enabled_cross_file_rules(config).any(|(_, _, needs_resolution)| needs_resolution)
}

fn resolve_rules(
    config: &FalconConfig,
) -> impl Iterator<Item = (&'static str, Box<dyn Rule>)> + '_ {
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
    for (name, make, _) in enabled_cross_file_rules(config) {
        registry.register(name, make());
    }
    registry
}

/// # Errors
///
/// Returns the spawn error when the rules thread cannot start.
pub fn run_cross_file(
    registry: &CrossFileRuleRegistry,
    files: &[ProjectFile],
    config: &FalconConfig,
    indexes: Option<&SemanticIndexes>,
) -> std::io::Result<Vec<Diagnostic>> {
    let mut diagnostics = registry.run_all(files, config, indexes)?;
    let files_by_path: HashMap<_, _> = files
        .iter()
        .map(|file| (file.path.to_string_lossy(), file))
        .collect();
    let mut suppressions = HashMap::new();
    diagnostics.retain(|diagnostic| {
        let Some((path, file)) = files_by_path.get_key_value(diagnostic.file_path.as_str()) else {
            return true;
        };
        let suppression = suppressions.entry(path.as_ref()).or_insert_with(|| {
            FileSuppressions::parse(file.source(), path, suppression_lookup, file.tree())
        });
        !suppression.is_suppressed(
            diagnostic.rule,
            suppression.line_for_offset(diagnostic.span.start),
        )
    });
    apply_severities(&mut diagnostics, config);
    Ok(diagnostics)
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

/// Return registry validation warnings without printing or constructing rules.
pub fn config_warnings(config: &FalconConfig) -> Vec<String> {
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
                    "{ctx} configures unknown rule `{name}` (under group `{group}`)"
                ));
                continue;
            };
            if meta.group.as_str() != group.as_str() {
                warnings.push(format!(
                    "{ctx} configures rule `{name}` under group `{group}`, \
                     but it belongs to `{}`",
                    meta.group.as_str()
                ));
                continue;
            }
            match (section, meta.is_cross_file()) {
                (Section::Linter, true) => warnings.push(format!(
                    "{ctx} configures `{name}` under `linter.rules`, but it is a \
                     cross-file rule; configure it under `cross-file.rules`"
                )),
                (Section::CrossFile, false) => warnings.push(format!(
                    "{ctx} configures `{name}` under `cross-file.rules`, but it is a \
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
            RuleKind::CrossFile { .. } => None,
        })
        .collect()
}

pub fn all_cross_file_rules() -> Vec<(&'static str, Box<dyn CrossFileRule>)> {
    RULE_METADATA
        .iter()
        .filter_map(|meta| match meta.kind {
            RuleKind::CrossFile { make, .. } => Some((meta.name, make())),
            RuleKind::File { .. } => None,
        })
        .collect()
}

type EnabledCrossFileRule = (&'static str, fn() -> Box<dyn CrossFileRule>, bool);

fn enabled_cross_file_rules(
    config: &FalconConfig,
) -> impl Iterator<Item = EnabledCrossFileRule> + '_ {
    RULE_METADATA.iter().filter_map(|meta| match meta.kind {
        RuleKind::CrossFile {
            make,
            needs_resolution,
        } if config.is_cross_file_rule_enabled_anywhere(
            meta.group.as_str(),
            meta.name,
            meta.recommended,
        ) =>
        {
            Some((meta.name, make, needs_resolution))
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
    fn config_warnings_use_security_group_for_secure_pubspec_urls() {
        let config = |group: &str| {
            serde_json::from_value::<FalconConfig>(serde_json::json!({
                "linter": { "rules": { "a11y": {} } },
                "cross-file": { "rules": {
                    "a11y": {},
                    group: { "secure-pubspec-urls": "error" }
                } }
            }))
            .expect("valid config")
        };
        assert!(config_warnings(&config("security")).is_empty());
        let warnings = config_warnings(&config("suspicious"));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("`secure-pubspec-urls`"));
        assert!(warnings[0].contains("belongs to `security`"));
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
    #[test]
    fn home_config_globs_follow_the_project_for_discovered_and_explicit_loads() {
        const CHILD: &str = "FALCON_TEST_HOME_CONFIG_ROOT";
        if std::env::var_os(CHILD).is_none() {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("project");
            std::fs::create_dir_all(&workspace).unwrap();
            std::fs::write(
                dir.path().join(".falcon.json"),
                r#"{
                    "files":{"includes":["lib/**", "!lib/generated/**"]},
                    "linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"warn"}}},
                    "overrides":[{
                        "includes":["lib/quiet/**"],
                        "linter":{"rules":{"suspicious":{"avoid-dynamic":"off"}}}
                    }]
                }"#,
            )
            .unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::home_config_globs_follow_the_project_for_discovered_and_explicit_loads",
                ])
                .env(CHILD, "1")
                .env("HOME", dir.path())
                .current_dir(&workspace)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let workspace = std::env::current_dir().unwrap();
        for loaded in [
            super::load_or_default(&workspace).unwrap(),
            super::load_config(std::path::Path::new("../.falcon.json")).unwrap(),
        ] {
            assert_eq!(loaded.config.root_dir(), Some(workspace.as_path()));
            for (relative, admitted, enabled) in [
                ("lib/main.dart", true, true),
                ("lib/generated/main.dart", false, true),
                ("lib/quiet/main.dart", true, false),
                ("test/main.dart", false, true),
            ] {
                let path = workspace.join(relative);
                let path = path.to_string_lossy();
                assert_eq!(
                    loaded.config.files.includes.admits(&path),
                    admitted,
                    "{relative}"
                );
                assert_eq!(
                    loaded
                        .config
                        .resolve_rule_for(&path, "suspicious", "avoid-dynamic", false, &[])
                        .is_some(),
                    enabled,
                    "{relative}"
                );
            }
        }
        let other_workspace = workspace.join("other");
        std::fs::create_dir_all(&other_workspace).unwrap();
        let loaded = super::load_or_default(&other_workspace).unwrap();
        assert_eq!(loaded.config.root_dir(), Some(other_workspace.as_path()));
        assert!(
            loaded
                .config
                .files
                .includes
                .admits(&other_workspace.join("lib/main.dart").to_string_lossy())
        );
        let local_config = other_workspace.join(".falcon.json");
        let home_config = workspace.parent().unwrap().join(".falcon.json");
        std::fs::copy(&home_config, &local_config).unwrap();
        let loaded = super::load_config(&local_config).unwrap();
        assert_eq!(loaded.config.root_dir(), Some(other_workspace.as_path()));
        assert!(
            loaded
                .config
                .files
                .includes
                .admits(&other_workspace.join("lib/main.dart").to_string_lossy())
        );

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&home_config, other_workspace.join("falcon.json")).unwrap();
            std::fs::create_dir(other_workspace.join(".git")).unwrap();
            let nested = other_workspace.join("nested");
            std::fs::create_dir(&nested).unwrap();
            let loaded = super::load_or_default(&nested).unwrap();
            assert_eq!(loaded.config.root_dir(), Some(other_workspace.as_path()));
            assert!(
                loaded
                    .config
                    .files
                    .includes
                    .admits(&other_workspace.join("lib/main.dart").to_string_lossy())
            );
        }
    }

    #[test]
    fn unified_loader_returns_legacy_path_and_registry_warnings() {
        let path = std::env::temp_dir().join(format!(
            "falcon-loader-warnings-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &path,
            serde_json::json!({
                "project": { "rules": { "correctness": { "unused-files": "warn" } } },
                "files": { "includes": ["["] },
                "linter": { "rules": {
                    "style": { "no_magic_number": "error", "no-magic-number": "warn" }
                } },
                "overrides": [{
                    "includes": ["**"],
                    "linter": { "rules": { "style": { "no_magic_number": "off" } } }
                }]
            })
            .to_string(),
        )
        .unwrap();
        let loaded = super::load_config(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(loaded.config.linter.rules.groups["style"].contains_key("no-magic-number"));
        assert!(!loaded.config.linter.rules.groups["style"].contains_key("no_magic_number"));
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("`project`"))
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("linter.rules.style.no_magic_number"))
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("overrides[0].linter.rules.style.no_magic_number"))
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("discarded"))
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("invalid") && warning.contains('['))
        );
        assert_eq!(loaded.warnings.len(), 5, "{:?}", loaded.warnings);
        assert!(
            loaded
                .warnings
                .iter()
                .all(|warning| !warning.starts_with("warning:"))
        );
    }

    #[test]
    fn loader_rejects_unknown_rule_ids_in_every_rule_map() {
        for (value, path) in [
            (
                serde_json::json!({ "linter": { "rules": { "style": { "typo": "off" } } } }),
                "linter.rules.style.typo",
            ),
            (
                serde_json::json!({ "cross-file": { "rules": { "style": { "typo": "off" } } } }),
                "cross-file.rules.style.typo",
            ),
            (
                serde_json::json!({ "overrides": [{ "linter": { "rules": { "style": { "typo": "off" } } } }] }),
                "overrides[0].linter.rules.style.typo",
            ),
            (
                serde_json::json!({ "overrides": [{ "cross-file": { "rules": { "style": { "typo": "off" } } } }] }),
                "overrides[0].cross-file.rules.style.typo",
            ),
        ] {
            let config = serde_json::from_value(value).unwrap();
            let loaded = falcon_config::LoadedConfig {
                config,
                warnings: Vec::new(),
            };
            let error = super::canonicalize_loaded_config(loaded).unwrap_err();
            assert!(error.0.contains(path), "{error}: expected {path}");
        }
    }
}
