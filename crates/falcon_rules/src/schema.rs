//! JSON Schema for falcon.json, derived from the rule metadata table.
//!
//! [`config_schema`] builds a draft-07 schema whose rule-name enums come
//! straight from [`RULE_METADATA`](crate::meta::RULE_METADATA), so editors that
//! resolve the config's `$schema` URL get rule autocomplete and level validation
//! for free. `cargo xtask schema` writes it to `schema/falcon.schema.json`; the
//! committed file's drift from this generator is guarded by `tests/meta_tests.rs`.

use serde_json::{Map, Value, json};

use crate::meta::{Domain, RULE_METADATA, RuleGroup};

/// Canonical published location of this release's configuration schema.
pub const SCHEMA_URL: &str = concat!(
    "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/v",
    env!("CARGO_PKG_VERSION"),
    "/schema/falcon.schema.json"
);

/// Rule option schemas generated from the same types used during config loading.
#[must_use]
pub fn rule_options_schemas() -> Vec<(&'static str, Value)> {
    crate::lint::options::schemas()
}

fn rule_config(options: &Value) -> Value {
    json!({
        "description": "A severity level, or an object with a severity level, rule-specific options and an optional autofix policy.",
        "oneOf": [
            {"$ref": "#/definitions/ruleLevel"},
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["level"],
                "properties": {
                    "level": {"$ref": "#/definitions/ruleLevel"},
                    "fix": {"$ref": "#/definitions/ruleFix"},
                    "options": {"anyOf": [options, {"type": "null"}]}
                }
            }
        ]
    })
}
/// The `rules` sub-schema for one section, with optional preset configuration.
/// Groups list exactly that section's rules (file rules unless `cross_file`).
fn section_rules(cross_file: bool, preset_allowed: bool) -> Value {
    let mut groups: Map<String, Value> = RuleGroup::ALL
        .iter()
        .map(|group| {
            (
                group.as_str().to_string(),
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {},
                    "description": format!(
                        "Per-rule severity levels for the `{}` group.",
                        group.as_str()
                    ),
                }),
            )
        })
        .collect();
    for meta in RULE_METADATA {
        if meta.is_cross_file() != cross_file {
            continue;
        }
        let entry = groups
            .get_mut(meta.group.as_str())
            .expect("registered rule group");
        entry["properties"]
            .as_object_mut()
            .expect("properties object")
            .insert(meta.name.to_string(), json!({
                "$ref": format!("#/definitions/{}", if meta.options.is_some() { meta.name } else { "ruleWithoutOptions" })
            }));
    }
    let mut props = Map::new();
    if preset_allowed {
        props.insert(
            "recommended".into(),
            json!({
                "type": ["boolean", "null"],
                "default": true,
                "description": "Enable the recommended preset for this section. \
                    When on, every rule in the preset reports at its default severity \
                    unless individually overridden. Defaults to true.",
            }),
        );
    }
    for (group, schema) in groups {
        props.insert(group, schema);
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": if preset_allowed {
            "Rule severity levels grouped by category, plus the `recommended` preset toggle."
        } else {
            "Explicit rule severity levels grouped by category."
        },
        "properties": Value::Object(props),
    })
}

/// Build the complete JSON Schema (draft-07) for falcon.json.
///
/// # Panics
///
/// Panics if a rule's registered group has no schema section.
#[must_use]
pub fn config_schema() -> Value {
    let mut domain_props = Map::new();
    for domain in Domain::ALL {
        let domain = domain.as_str();
        domain_props.insert(
            domain.to_string(),
            json!({
                "enum": ["all", "recommended", "none"],
                "default": "recommended",
                "description": format!(
                    "Activation level for the `{domain}` domain's rules: `all` enables \
                     every rule in the domain, `recommended` only its recommended rules, \
                     `none` disables them."
                ),
            }),
        );
    }

    let mut schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": SCHEMA_URL,
        "title": "falcon.json",
        "description": "Configuration for the falcon Dart linter.",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "$schema": {
                "type": ["string", "null"],
                "description": "URL of the falcon.json JSON Schema, enabling rule \
                    autocomplete and level validation in editors."
            },
            "baseline": {
                "type": ["string", "null"],
                "description": "Diagnostic baseline path. Relative paths resolve from the \
                    configuration files root (the current directory for user-level config). \
                    The CLI --baseline path takes precedence. Omit or use null for no baseline."
            },
            "error-on-warnings": {
                "type": "boolean",
                "default": false,
                "description": "Fail checks on warnings as well as errors. By default, \
                    warning-only checks succeed. CLI --error-on-warnings enables this setting."
            },
            "files": {
                "type": "object",
                "additionalProperties": false,
                "description": "Which files falcon discovers and analyzes.",
                "properties": {
                    "includes": string_globs(
                        "Glob patterns relative to the configuration directory. \
                         A single star stops at directory separators; a double star \
                         crosses them. When omitted, every Dart file is a candidate."
                    )
                }
            },
            "linter": linter_schema(domain_props),
            "cross-file": cross_file_schema(),
            "overrides": overrides_schema(),
            "max-errors": {
                "type": ["integer", "null"],
                "minimum": 1,
                "default": null,
                "description": "Print at most this many diagnostics without changing \
                    the exit code. Null (the default) means no limit. Must be at \
                    least 1 in configuration; use the CLI flag to hide all output."
            }
        },
        "definitions": schema_definitions()
    });
    let definitions = schema["definitions"]
        .as_object_mut()
        .expect("schema definitions");
    definitions.insert(
        "ruleWithoutOptions".into(),
        rule_config(&json!({
            "type": "object", "additionalProperties": false, "properties": {}
        })),
    );
    for (name, options) in rule_options_schemas() {
        definitions.insert(name.into(), rule_config(&options));
    }
    schema
}

fn string_globs(desc: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "description": desc,
    })
}

fn linter_schema(domain_props: Map<String, Value>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Single-file linter configuration.",
        "properties": {
            "enabled": {
                "type": "boolean",
                "default": true,
                "description": "Master switch for the single-file linter. \
                    Set to false to disable all file rules. Defaults to true."
            },
            "domains": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "description": "Per-domain gating for domain-scoped rules \
                    (for example Flutter rules).",
                "properties": Value::Object(domain_props)
            },
            "rules": section_rules(false, true)
        }
    })
}

fn cross_file_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Cross-file (whole-project) analysis configuration. \
            These rules scan every file and are slower than single-file rules.",
        "properties": {
            "enabled": {
                "type": "boolean",
                "default": true,
                "description": "Master switch for cross-file analysis. Set to \
                    false to disable all cross-file rules. Defaults to true."
            },
            "rules": section_rules(true, true)
        }
    })
}

fn overrides_schema() -> Value {
    json!({
        "type": "array",
        "description": "Per-glob configuration overrides, applied in order to \
            files matching their `includes`.",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "description": "A single override: an `includes` glob set plus the \
                linter/cross-file settings to apply to the matched files.",
            "properties": {
                "includes": string_globs(
                    "Glob patterns relative to the configuration directory \
                     selecting the files this override applies to. Omitted \
                     patterns apply to every file."
                ),
                "linter": {
                    "anyOf": [
                        {"$ref": "#/definitions/overrideLinter"},
                        {"type": "null"}
                    ]
                },
                "cross-file": {
                    "anyOf": [
                        {"$ref": "#/definitions/overrideCrossFile"},
                        {"type": "null"}
                    ]
                }
            }
        }
    })
}

fn schema_definitions() -> Value {
    json!({
        "ruleLevel": {
            "enum": ["off", "on", "info", "warn", "error"],
            "description": "Severity level for a rule: `off` disables it; `on` \
                reports at its default severity; `info`, `warn`, and `error` set an \
                explicit severity."
        },
        "ruleFix": {
            "enum": ["none", "safe", "unsafe"],
            "description": "Autofix policy: `none` removes fixes; `safe` or `unsafe` \
                sets their safety classification. When omitted, inherit the previous \
                matching policy or the rule's default classification."
        },
        "overrideLinter": {
            "type": "object",
            "additionalProperties": false,
            "description": "Linter settings applied by an override to its matched files.",
            "properties": {
                "enabled": {
                    "type": ["boolean", "null"],
                    "description": "Master switch for the single-file linter within \
                        this override."
                },
                "rules": section_rules(false, false)
            }
        },
        "overrideCrossFile": {
            "type": "object",
            "additionalProperties": false,
            "description": "Cross-file settings applied by an override to its matched files.",
            "properties": {
                "enabled": {
                    "type": ["boolean", "null"],
                    "description": "Master switch for cross-file analysis within this \
                        override."
                },
                "rules": section_rules(true, false)
            }
        }
    })
}

/// The schema serialized exactly as it is written to disk: pretty-printed with a
/// trailing newline. Shared by `cargo xtask schema` (writer) and the drift test.
///
/// # Panics
///
/// Panics if serializing the schema fails.
#[must_use]
pub fn config_schema_string() -> String {
    let mut s = serde_json::to_string_pretty(&config_schema()).expect("schema serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoption_settings_have_canonical_types_and_defaults() {
        let schema = config_schema();
        let properties = &schema["properties"];
        assert_eq!(properties["baseline"]["type"], json!(["string", "null"]));
        assert_eq!(properties["error-on-warnings"]["type"], "boolean");
        assert_eq!(properties["error-on-warnings"]["default"], false);
        for domain in Domain::ALL {
            assert_eq!(
                properties["linter"]["properties"]["domains"]["properties"][domain.as_str()]
                    ["default"],
                "recommended"
            );
        }
        assert!(properties.get("error_on_warnings").is_none());
        let overrides = &properties["overrides"]["items"]["properties"];
        assert!(overrides.get("baseline").is_none());
        assert!(overrides.get("error-on-warnings").is_none());
    }

    #[test]
    fn every_rule_object_accepts_an_optional_fix_policy() {
        let schema = config_schema();
        assert_eq!(
            schema["definitions"]["ruleFix"]["enum"],
            json!(["none", "safe", "unsafe"])
        );
        for name in std::iter::once("ruleWithoutOptions")
            .chain(rule_options_schemas().iter().map(|(name, _)| *name))
        {
            let object = &schema["definitions"][name]["oneOf"][1];
            assert_eq!(object["properties"]["fix"]["$ref"], "#/definitions/ruleFix");
            assert_eq!(object["required"], json!(["level"]));
        }
    }

    #[test]
    fn optional_configuration_fields_accept_null() {
        let schema = config_schema();
        for path in [
            "/properties/$schema/type",
            "/properties/linter/properties/domains/type",
            "/properties/linter/properties/rules/properties/recommended/type",
            "/properties/cross-file/properties/rules/properties/recommended/type",
            "/definitions/overrideLinter/properties/enabled/type",
            "/definitions/overrideCrossFile/properties/enabled/type",
            "/properties/max-errors/type",
            "/properties/baseline/type",
        ] {
            assert!(
                schema
                    .pointer(path)
                    .and_then(Value::as_array)
                    .is_some_and(|types| types.contains(&json!("null"))),
                "{path}"
            );
        }
        for section in ["linter", "cross-file"] {
            assert_eq!(
                schema["properties"]["overrides"]["items"]["properties"][section]["anyOf"][1],
                json!({"type": "null"})
            );
        }
    }
}
