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
