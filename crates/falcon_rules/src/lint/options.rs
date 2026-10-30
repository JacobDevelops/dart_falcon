//! Validated rule options cached with each distinct configuration value.

use falcon_analyze::AnalyzeContext;
use falcon_config::{
    ConfigError, ConfigPathSegment, FalconConfig, RuleOptions, deserialize_error_path,
    format_config_path,
};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

pub(crate) trait TypedOptions:
    DeserializeOwned + JsonSchema + Send + Sync + 'static
{
    fn defaults() -> &'static Self;
}

macro_rules! impl_options {
    ($options:ty) => {
        impl crate::lint::options::TypedOptions for $options {
            fn defaults() -> &'static Self {
                static DEFAULT: std::sync::LazyLock<$options> =
                    std::sync::LazyLock::new(<$options>::default);
                &DEFAULT
            }
        }
    };
}
pub(crate) use impl_options;

pub(crate) fn get<'a, T: TypedOptions>(ctx: &'a AnalyzeContext, name: &str) -> &'a T {
    let meta = crate::meta::meta_for(name).expect("registered option-reading rule");
    let options = ctx.rule_options_data(meta.group.as_str(), meta.name);
    read(options).unwrap_or_else(|error| {
        let path = ctx.rule_options_location(meta.group.as_str(), meta.name);
        ctx.report_config_error(option_error(&path, error).0);
        T::defaults()
    })
}

pub(crate) fn cross_file<'a, T: TypedOptions>(
    config: &'a FalconConfig,
    path: &str,
    name: &str,
) -> Result<&'a T, ConfigError> {
    let meta = crate::meta::meta_for(name).expect("registered option-reading rule");
    let options = config.cross_file_options_data_for(path, meta.group.as_str(), meta.name);
    read(options).map_err(|error| {
        let path = config.cross_file_options_location_for(path, meta.group.as_str(), meta.name);
        option_error(&path, error)
    })
}

struct OptionError {
    path: Vec<ConfigPathSegment>,
    message: String,
}

fn read<T: TypedOptions>(options: Option<&RuleOptions>) -> Result<&T, &OptionError> {
    match options {
        Some(options) => options.get_or_try_init(|value| {
            if !value.is_object() {
                return Err(OptionError {
                    path: Vec::new(),
                    message: "expected an options object".to_owned(),
                });
            }
            T::deserialize(value).map_err(|error| OptionError {
                path: deserialize_error_path::<T>(value, &[]),
                message: error.to_string(),
            })
        }),
        None => Ok(T::defaults()),
    }
}

fn option_error(path: &[ConfigPathSegment], error: &OptionError) -> ConfigError {
    let mut path = path.to_vec();
    path.extend_from_slice(&error.path);
    ConfigError::at(
        format!(
            "invalid rule options `{}`: {}",
            format_config_path(&path),
            error.message
        ),
        path,
    )
}

/// A regex compiled when its containing rule options are loaded.
#[derive(Debug, Clone, JsonSchema)]
#[schemars(with = "String")]
pub(crate) struct Regex(regex::Regex);

impl Regex {
    pub(crate) fn new(pattern: &str) -> Result<Self, regex::Error> {
        regex::Regex::new(pattern).map(Self)
    }
}

impl std::ops::Deref for Regex {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Regex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pattern = String::deserialize(deserializer)?;
        Self::new(&pattern).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Regex {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A validated glob pattern used to match a banned name, API, or import.
#[derive(Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub(crate) struct Glob(glob::Pattern);

impl TryFrom<String> for Glob {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("glob must not be empty".into());
        }
        glob::Pattern::new(&value)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

impl From<Glob> for String {
    fn from(value: Glob) -> Self {
        value.0.as_str().to_owned()
    }
}

impl Glob {
    pub(crate) fn matches_with(&self, value: &str, options: glob::MatchOptions) -> bool {
        self.0.matches_with(value, options)
    }
}

pub(crate) fn nonempty_patterns<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Glob>, D::Error> {
    let patterns = Vec::<Glob>::deserialize(deserializer)?;
    if patterns.is_empty() {
        return Err(serde::de::Error::custom("glob list must not be empty"));
    }
    Ok(patterns)
}

pub(crate) fn nonblank_message<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let message = String::deserialize(deserializer)?;
    if message.trim().is_empty() {
        return Err(serde::de::Error::custom("message must not be blank"));
    }
    Ok(message)
}

fn schema<T: TypedOptions>() -> Value {
    let settings = SchemaSettings::draft07().with(|settings| settings.inline_subschemas = true);
    let mut schema = serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
        .expect("options schema serializes");
    schema
        .as_object_mut()
        .expect("options object schema")
        .remove("$schema");
    schema
}

fn validate<T: TypedOptions>(
    options: &RuleOptions,
    path: &[ConfigPathSegment],
) -> Result<(), ConfigError> {
    read::<T>(Some(options))
        .map(|_| ())
        .map_err(|error| option_error(path, error))
}

#[derive(Clone, Copy)]
pub(crate) struct OptionDefinition {
    schema: fn() -> Value,
    validate: fn(&RuleOptions, &[ConfigPathSegment]) -> Result<(), ConfigError>,
}

impl OptionDefinition {
    pub(crate) const fn new<T: TypedOptions>() -> Self {
        Self {
            schema: schema::<T>,
            validate: validate::<T>,
        }
    }
}

pub(crate) fn schemas() -> Vec<(&'static str, Value)> {
    let mut schemas: Vec<_> = crate::meta::RULE_METADATA
        .iter()
        .filter_map(|meta| schema_for(meta.name).map(|schema| (meta.name, schema.clone())))
        .collect();
    schemas.sort_unstable_by_key(|(name, _)| *name);
    schemas
}

pub(crate) fn schema_for(name: &str) -> Option<&'static Value> {
    use std::{
        collections::HashMap,
        sync::{LazyLock, OnceLock},
    };
    static SCHEMAS: LazyLock<HashMap<&'static str, OnceLock<Value>>> = LazyLock::new(|| {
        crate::meta::RULE_METADATA
            .iter()
            .filter(|meta| meta.options.is_some())
            .map(|meta| (meta.name, OnceLock::new()))
            .collect()
    });
    let meta = crate::meta::meta_for(name)?;
    let definition = meta.options?;
    let slot = SCHEMAS.get(meta.name)?;
    Some(slot.get_or_init(|| (definition.schema)()))
}

pub(crate) fn validate_rule(
    name: &str,
    options: &RuleOptions,
    path: &[ConfigPathSegment],
) -> Result<(), ConfigError> {
    if let Some(definition) = crate::meta::meta_for(name).and_then(|meta| meta.options) {
        return (definition.validate)(options, path);
    }
    let value = options.value();
    if value.as_object().is_some_and(serde_json::Map::is_empty) {
        return Ok(());
    }
    let mut path = path.to_vec();
    if let Some(key) = value.as_object().and_then(|object| object.keys().next()) {
        path.push(ConfigPathSegment::Key(key.clone()));
    }
    Err(ConfigError::at(
        format!(
            "invalid rule options `{}`: this rule has no options",
            format_config_path(&path)
        ),
        path,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integer_options_accept_whole_number_floats() {
        for (name, schema) in schemas() {
            for (key, field) in schema["properties"].as_object().unwrap() {
                let integer = field["type"] == "integer"
                    || field["type"]
                        .as_array()
                        .is_some_and(|types| types.iter().any(|kind| kind == "integer"));
                if !integer {
                    continue;
                }
                let options: RuleOptions = serde_json::from_value(json!({key: 1.0})).unwrap();
                validate_rule(name, &options, &[])
                    .unwrap_or_else(|error| panic!("{name}.{key}: {error}"));
            }
        }
    }

    #[test]
    fn integer_options_report_allowed_range() {
        for input in ["-1", "1.5", "18446744073709551616", "1e100"] {
            let options: RuleOptions =
                serde_json::from_str(&format!(r#"{{"max-lines": {input}}}"#)).unwrap();
            let error = validate_rule("max-lines-for-file", &options, &[]).unwrap_err();
            assert!(error.0.contains("max-lines"), "{error}");
            assert!(
                error
                    .0
                    .contains(&format!("allowed range 0..={}", usize::MAX)),
                "{error}"
            );
            assert_eq!(error.1, [ConfigPathSegment::Key("max-lines".into())]);
        }
    }

    #[test]
    fn integer_options_preserve_exact_maximum() {
        let options: RuleOptions =
            serde_json::from_value(json!({"max-lines": usize::MAX})).unwrap();
        validate_rule("max-lines-for-file", &options, &[]).unwrap();
    }

    #[test]
    fn optional_integer_accepts_null() {
        let options: RuleOptions =
            serde_json::from_value(json!({"allowed-line-count": null})).unwrap();
        validate_rule("prefer-extracting-callbacks", &options, &[]).unwrap();
    }

    #[test]
    fn option_error_paths_preserve_literal_unknown_keys() {
        let base = [ConfigPathSegment::Key("options".into())];
        for key in ["a.b", "a[0]", "a`b"] {
            let options: RuleOptions = serde_json::from_value(json!({key: true})).unwrap();
            let error = validate_rule("max-lines-for-file", &options, &base).unwrap_err();
            assert_eq!(
                error.1,
                [
                    ConfigPathSegment::Key("options".into()),
                    ConfigPathSegment::Key(key.into())
                ]
            );
        }
    }

    #[test]
    fn option_error_paths_preserve_nested_array_indices() {
        let options: RuleOptions = serde_json::from_value(json!({
            "rules": [{"deny": ["valid", 42], "message": "Banned."}]
        }))
        .unwrap();
        let error = validate_rule("avoid-banned-names", &options, &[]).unwrap_err();
        assert_eq!(
            error.1,
            [
                ConfigPathSegment::Key("rules".into()),
                ConfigPathSegment::Index(0),
                ConfigPathSegment::Key("deny".into()),
                ConfigPathSegment::Index(1)
            ]
        );
    }

    #[test]
    fn whole_number_float_threshold_controls_rule_behavior() {
        use falcon_analyze::{ParsedFile, RuleAnalysis};

        let config = serde_json::from_value(json!({
            "linter": {"rules": {"complexity": {"max-lines-for-file": {
                "level": "warn", "options": {"max-lines": 1.0}
            }}}}
        }))
        .unwrap();
        let source = "void main() {\n}\n";
        let parsed = std::sync::Arc::new(ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parser::parse(source).tree,
        });
        let context = AnalyzeContext::new(std::path::Path::new("main.dart"), &config, &parsed);
        let crate::meta::RuleKind::File { make, .. } =
            crate::meta::meta_for("max-lines-for-file").unwrap().kind
        else {
            unreachable!()
        };
        let diagnostics = make().analyze("max-lines-for-file", parsed.program(), &context);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, "max-lines-for-file");
    }

    fn load_value(value: &Value) -> Result<falcon_config::LoadedConfig, ConfigError> {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("falcon.json");
        std::fs::write(&path, value.to_string()).unwrap();
        crate::load_config(&path)
    }

    #[test]
    fn loaded_option_errors_preserve_original_alias_paths() {
        for in_override in [false, true] {
            for (group, rule, key, invalid, index) in [
                (
                    "complexity",
                    "max-lines-for-file",
                    "max_lines",
                    json!(-1),
                    None,
                ),
                (
                    "style",
                    "boolean_prefixes",
                    "valid_prefixes",
                    json!(["ok", 42]),
                    Some(1),
                ),
            ] {
                let section = json!({"rules": {group: {rule: {
                    "level": "warn", "options": {key: invalid}
                }}}});
                let value = if in_override {
                    json!({"overrides": [{"linter": section}]})
                } else {
                    json!({"linter": section})
                };
                let error = load_value(&value).unwrap_err();
                let mut expected = if in_override {
                    vec![
                        ConfigPathSegment::Key("overrides".into()),
                        ConfigPathSegment::Index(0),
                    ]
                } else {
                    Vec::new()
                };
                expected.extend(
                    ["linter", "rules", group, rule, "options", key]
                        .map(|key| ConfigPathSegment::Key(key.into())),
                );
                if let Some(index) = index {
                    expected.push(ConfigPathSegment::Index(index));
                }
                assert_eq!(error.1, expected);
                assert!(error.0.contains(&format_config_path(&expected)), "{error}");
            }
        }
    }

    #[test]
    fn discarded_rule_and_option_aliases_do_not_fail_validation() {
        let loaded = load_value(&json!({
            "linter": {"rules": {
                "style": {
                    "boolean_prefixes": {"level": "warn", "options": {"valid_prefixes": 42}},
                    "boolean-prefixes": {"level": "warn", "options": {"valid-prefixes": ["is"]}}
                },
                "complexity": {"max-lines-for-file": {
                    "level": "warn", "options": {"max_lines": -1, "max-lines": 1}
                }}
            }}
        }))
        .unwrap();
        assert!(!loaded.config.linter.rules.groups["style"].contains_key("boolean_prefixes"));
        let options = loaded.config.linter.rules.groups["complexity"]["max-lines-for-file"]
            .options_data()
            .unwrap();
        assert_eq!(options.value(), &json!({"max-lines": 1}));
    }

    #[test]
    fn winning_higher_severity_alias_reports_its_original_path() {
        let error = load_value(&json!({
            "linter": {"rules": {"style": {
                "boolean_prefixes": {"level": "error", "options": {"valid_prefixes": 42}},
                "boolean-prefixes": {"level": "warn", "options": {"valid-prefixes": ["is"]}}
            }}}
        }))
        .unwrap_err();
        assert_eq!(
            error.1,
            [
                "linter",
                "rules",
                "style",
                "boolean_prefixes",
                "options",
                "valid_prefixes"
            ]
            .map(|key| ConfigPathSegment::Key(key.into()))
        );
    }
}
