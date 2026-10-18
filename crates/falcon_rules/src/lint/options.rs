//! Typed readers for per-file rule options, including path overrides.

use falcon_analyze::AnalyzeContext;
use serde_json::Value;

use crate::meta::{RuleMeta, meta_for};

pub(crate) fn for_rule<'a>(ctx: &'a AnalyzeContext, name: &str) -> Option<&'a Value> {
    for_meta(ctx, meta_for(name)?)
}

fn for_meta<'a>(ctx: &'a AnalyzeContext, meta: &RuleMeta) -> Option<&'a Value> {
    ctx.rule_options(meta.group.as_str(), meta.name)
}

pub(crate) fn cross_file_for_rule<'a>(
    config: &'a falcon_config::FalconConfig,
    path: &str,
    name: &str,
) -> Option<&'a Value> {
    let group = meta_for(name)?.group.as_str();
    let mut result = config
        .cross_file
        .rules
        .groups
        .get(group)
        .and_then(|rules| rules.get(name))
        .and_then(falcon_config::RuleConfiguration::options);
    for entry in &config.overrides {
        if !entry.matches(path) {
            continue;
        }
        if let Some(options) = entry
            .cross_file
            .as_ref()
            .and_then(|section| section.rules.groups.get(group))
            .and_then(|rules| rules.get(name))
            .and_then(falcon_config::RuleConfiguration::options)
        {
            result = Some(options);
        }
    }
    result
}

pub(crate) fn usize(options: Option<&Value>, key: &str) -> Option<usize> {
    options?.get(key)?.as_u64().map(|value| value as usize)
}

pub(crate) fn strings<'a>(
    options: Option<&'a Value>,
    key: &str,
) -> Option<impl Iterator<Item = &'a str>> {
    Some(
        options?
            .get(key)?
            .as_array()?
            .iter()
            .filter_map(Value::as_str),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cross_file_options_use_the_cross_file_section_and_path_overrides() {
        let config = serde_json::from_value(json!({
            "linter": {"rules": {"correctness": {
                "unused-files": {"level": "warn", "options": {"origin": "linter"}}
            }}},
            "cross-file": {"rules": {"correctness": {
                "unused-files": {"level": "warn", "options": {"origin": "base"}}
            }}},
            "overrides": [
                {"includes": ["lib/**"], "cross-file": {"rules": {"correctness": {
                    "unused-files": {"level": "warn", "options": {"origin": "override"}}
                }}}},
                {"includes": ["lib/**"], "cross-file": {"rules": {"correctness": {
                    "unused-files": "error"
                }}}}
            ]
        }))
        .unwrap();
        assert_eq!(
            cross_file_for_rule(&config, "test/test.dart", "unused-files"),
            Some(&json!({"origin": "base"}))
        );
        assert_eq!(
            cross_file_for_rule(&config, "lib/main.dart", "unused-files"),
            Some(&json!({"origin": "override"}))
        );
        assert!(cross_file_for_rule(&config, "lib/main.dart", "unknown-rule").is_none());
    }

    #[test]
    fn usize_preserves_lenient_defaults() {
        assert_eq!(usize(None, "limit").unwrap_or(20), 20);
        for options in [
            json!({}),
            json!({"limit": null}),
            json!({"limit": -1}),
            json!({"limit": 1.5}),
            json!({"limit": "3"}),
        ] {
            assert_eq!(usize(Some(&options), "limit").unwrap_or(20), 20);
        }
        for value in [0, 3, 20] {
            assert_eq!(usize(Some(&json!({"limit": value})), "limit"), Some(value));
        }
    }

    #[test]
    fn strings_preserves_missing_empty_and_mixed_lists() {
        assert!(strings(None, "order").is_none());
        assert!(strings(Some(&json!({"order": "fields"})), "order").is_none());
        let empty = json!({"order": []});
        assert_eq!(strings(Some(&empty), "order").unwrap().count(), 0);
        let mixed = json!({"order": ["fields", null, 3, "methods"]});
        assert_eq!(
            strings(Some(&mixed), "order").unwrap().collect::<Vec<_>>(),
            ["fields", "methods"],
        );
    }
}
