//! Validated rule options cached with each distinct configuration value.

use falcon_analyze::AnalyzeContext;
use falcon_config::{ConfigError, FalconConfig, RuleOptions};
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
    read(ctx.rule_options_data(meta.group.as_str(), meta.name))
}

pub(crate) fn cross_file<'a, T: TypedOptions>(
    config: &'a FalconConfig,
    path: &str,
    name: &str,
) -> &'a T {
    let meta = crate::meta::meta_for(name).expect("registered option-reading rule");
    read(config.cross_file_options_data_for(path, meta.group.as_str(), meta.name))
}

fn read<T: TypedOptions>(options: Option<&RuleOptions>) -> &T {
    match options {
        Some(options) => options.get_or_init(|value| {
            serde_json::from_value(value.clone())
                .expect("rule options validated during config loading")
        }),
        None => T::defaults(),
    }
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
    options
        .get_or_try_init(|value| serde_json::from_value::<T>(value.clone()))
        .map(|_| ())
        .map_err(|error| {
            let path = error_path::<T>(options.value(), path);
            ConfigError(format!("invalid rule options `{path}`: {error}"))
        })
}

fn error_path<T: DeserializeOwned>(value: &Value, path: &str) -> String {
    let mut source = String::new();
    let mut locations = Vec::new();
    write_option_locations(value, path, &mut source, &mut locations);
    let Err(error) = serde_json::from_str::<T>(&source) else {
        return path.to_owned();
    };
    let offset = error.column().saturating_sub(1);
    // Missing-field errors consume the closing brace, not the last property's value.
    let missing_field = error.to_string().starts_with("missing field ");
    locations
        .into_iter()
        .rev()
        .find(|(start, end, _)| {
            *start <= offset
                && if missing_field {
                    offset < *end
                } else {
                    offset <= *end
                }
        })
        .map_or_else(|| path.to_owned(), |(_, _, path)| path)
}

fn write_option_locations(
    value: &Value,
    path: &str,
    source: &mut String,
    locations: &mut Vec<(usize, usize, String)>,
) {
    let start = source.len();
    let location = locations.len();
    locations.push((start, start, path.to_owned()));
    match value {
        Value::Object(entries) => {
            source.push('{');
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    source.push(',');
                }
                let path = format!("{path}.{key}");
                let key_start = source.len();
                source.push_str(&serde_json::to_string(key).expect("option key serializes"));
                locations.push((key_start, source.len(), path.clone()));
                source.push(':');
                write_option_locations(value, &path, source, locations);
            }
            source.push('}');
        }
        Value::Array(entries) => {
            source.push('[');
            for (index, value) in entries.iter().enumerate() {
                if index > 0 {
                    source.push(',');
                }
                write_option_locations(value, &format!("{path}[{index}]"), source, locations);
            }
            source.push(']');
        }
        _ => source.push_str(&serde_json::to_string(value).expect("option value serializes")),
    }
    locations[location].1 = source.len();
}

struct OptionDefinition {
    name: &'static str,
    schema: fn() -> Value,
    validate: fn(&RuleOptions, &str) -> Result<(), ConfigError>,
}

macro_rules! option_definitions {
    ($($name:literal => $options:ty),* $(,)?) => {
        const DEFINITIONS: &[OptionDefinition] = &[$(
            OptionDefinition { name: $name, schema: schema::<$options>, validate: validate::<$options> },
        )*];
    };
}

use crate::lint::{complexity, correctness, performance, style, suspicious};

option_definitions! {
    "avoid-complex-arithmetic-expressions" => complexity::avoid_complex_arithmetic_expressions::Options,
    "avoid-complex-conditions" => complexity::avoid_complex_conditions::Options,
    "avoid-complex-loop-conditions" => complexity::avoid_complex_loop_conditions::Options,
    "avoid-if-with-many-branches" => complexity::avoid_if_with_many_branches::Options,
    "avoid-long-records" => complexity::avoid_long_records::Options,
    "avoid-nested-switch-expressions" => complexity::avoid_nested_switch_expressions::Options,
    "avoid-nested-switches" => complexity::avoid_nested_switches::Options,
    "avoid-nested-try-statements" => complexity::avoid_nested_try_statements::Options,
    "cognitive-complexity" => complexity::cognitive_complexity::Options,
    "cyclomatic-complexity" => complexity::cyclomatic_complexity::Options,
    "max-lines-for-file" => complexity::max_lines_for_file::Options,
    "max-lines-for-function" => complexity::max_lines_for_function::Options,
    "max-parameters-for-function" => complexity::max_parameters_for_function::Options,
    "max-statements" => complexity::max_statements::Options,
    "max-switch-cases" => complexity::max_switch_cases::Options,
    "maximum-nesting-level" => complexity::maximum_nesting_level::Options,
    "prefer-extracting-callbacks" => complexity::prefer_extracting_callbacks::Options,
    "prefer-moving-to-variable" => complexity::prefer_moving_to_variable::Options,
    "widget-tree-depth" => complexity::widget_tree_depth::Options,
    "avoid-empty-test-groups" => correctness::avoid_empty_test_groups::Options,
    "missing-test-assertion" => correctness::missing_test_assertion::Options,
    "move-variable-closer-to-its-usage" => performance::move_variable_closer_to_its_usage::Options,
    "arguments-ordering" => style::arguments_ordering::Options,
    "avoid-unnecessary-getter" => style::avoid_unnecessary_getter::Options,
    "boolean-prefixes" => style::boolean_prefixes::Options,
    "class-members-ordering" => style::class_members_ordering::Options,
    "enum-constants-ordering" => style::enum_constants_ordering::Options,
    "extend-equatable" => style::extend_equatable::Options,
    "format-comment" => style::format_comment::Options,
    "initializers-ordering" => style::initializers_ordering::Options,
    "map-keys-ordering" => style::map_keys_ordering::Options,
    "member-ordering" => style::member_ordering::Options,
    "no-magic-number" => style::no_magic_number::Options,
    "parameters-ordering" => style::parameters_ordering::Options,
    "pattern-fields-ordering" => style::pattern_fields_ordering::Options,
    "prefer-commenting-analyzer-ignores" => style::prefer_commenting_analyzer_ignores::Options,
    "prefer-container" => style::prefer_container::Options,
    "prefer-correct-callback-field-name" => style::prefer_correct_callback_field_name::Options,
    "prefer-correct-error-name" => style::prefer_correct_error_name::Options,
    "prefer-correct-handler-name" => style::prefer_correct_handler_name::Options,
    "prefer-correct-identifier-length" => style::prefer_correct_identifier_length::Options,
    "prefer-named-parameters" => style::prefer_named_parameters::Options,
    "record-fields-ordering" => style::record_fields_ordering::Options,
    "use-design-system-item" => style::use_design_system_item::Options,
    "avoid-banned-annotations" => suspicious::avoid_banned_annotations::Options,
    "avoid-banned-exports" => suspicious::avoid_banned_exports::Options,
    "avoid-banned-names" => suspicious::avoid_banned_names::Options,
    "avoid-banned-types" => suspicious::avoid_banned_types::Options,
    "avoid-negated-conditions" => suspicious::avoid_negated_conditions::Options,
    "avoid-nested-assignments" => suspicious::avoid_nested_assignments::Options,
    "avoid-referencing-subclasses" => suspicious::avoid_referencing_subclasses::Options,
    "banned-usage" => suspicious::banned_usage::Options,
    "no-magic-string" => suspicious::no_magic_string::Options,
    "prefer-unique-test-names" => suspicious::prefer_unique_test_names::Options,
    "banned-imports" => crate::cross_file::banned_imports::Options,
}

pub(crate) fn schemas() -> Vec<(&'static str, Value)> {
    let mut schemas: Vec<_> = DEFINITIONS
        .iter()
        .map(|entry| (entry.name, (entry.schema)()))
        .collect();
    schemas.sort_unstable_by_key(|(name, _)| *name);
    schemas
}

pub(crate) fn schema_for(name: &str) -> Option<Value> {
    DEFINITIONS
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| (entry.schema)())
}

pub(crate) fn validate_rule(
    name: &str,
    options: &RuleOptions,
    path: &str,
) -> Result<(), ConfigError> {
    if let Some(definition) = DEFINITIONS.iter().find(|entry| entry.name == name) {
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
