//! Validated rule options cached with each distinct configuration value.

use falcon_analyze::AnalyzeContext;
use falcon_config::{ConfigError, FalconConfig, RuleOptions, deserialize_error_path};
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
    path: String,
    message: String,
}

fn read<T: TypedOptions>(options: Option<&RuleOptions>) -> Result<&T, &OptionError> {
    match options {
        Some(options) => options.get_or_try_init(|value| {
            if !value.is_object() {
                return Err(OptionError {
                    path: String::new(),
                    message: "expected an options object".to_owned(),
                });
            }
            T::deserialize(value).map_err(|error| OptionError {
                path: deserialize_error_path::<T>(value, ""),
                message: error.to_string(),
            })
        }),
        None => Ok(T::defaults()),
    }
}

fn option_error(path: &str, error: &OptionError) -> ConfigError {
    let separator = if error.path.is_empty() || error.path.starts_with('[') {
        ""
    } else {
        "."
    };
    ConfigError(format!(
        "invalid rule options `{path}{separator}{}`: {}",
        error.path, error.message
    ))
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

fn validate<T: TypedOptions>(options: &RuleOptions, path: &str) -> Result<(), ConfigError> {
    read::<T>(Some(options))
        .map(|_| ())
        .map_err(|error| option_error(path, error))
}

#[derive(Clone, Copy)]
pub(crate) struct OptionDefinition {
    schema: fn() -> Value,
    validate: fn(&RuleOptions, &str) -> Result<(), ConfigError>,
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
    path: &str,
) -> Result<(), ConfigError> {
    if let Some(definition) = crate::meta::meta_for(name).and_then(|meta| meta.options) {
        return (definition.validate)(options, path);
    }
    let value = options.value();
    if value.as_object().is_some_and(serde_json::Map::is_empty) {
        return Ok(());
    }
    let path = value
        .as_object()
        .and_then(|object| object.keys().next())
        .map_or_else(|| path.to_owned(), |key| format!("{path}.{key}"));
    Err(ConfigError(format!(
        "invalid rule options `{path}`: this rule has no options"
    )))
}
