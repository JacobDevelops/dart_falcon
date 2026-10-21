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
pub fn rule_options_schemas() -> Vec<(&'static str, Value)> {
    crate::lint::options::schemas()
}

fn rule_config(options: Value) -> Value {
    json!({
        "description": "A severity level, or an object with a severity level and rule-specific options.",
        "oneOf": [
            {"$ref": "#/definitions/ruleLevel"},
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["level"],
                "properties": {
                    "level": {"$ref": "#/definitions/ruleLevel"},
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
pub fn config_schema() -> Value {
    let mut domain_props = Map::new();
    for domain in Domain::ALL {
        let domain = domain.as_str();
        domain_props.insert(
            domain.to_string(),
            json!({
                "enum": ["all", "recommended", "none"],
                "description": format!(
                    "Activation level for the `{domain}` domain's rules: `all` enables \
                     every rule in the domain, `recommended` only its recommended rules, \
                     `none` disables them."
                ),
            }),
        );
    }
    let string_globs = |desc: &str| {
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": desc,
        })
    };

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
            "linter": {
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
            },
            "cross-file": {
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
            },
            "overrides": {
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
            },
            "max-errors": {
                "type": ["integer", "null"],
                "minimum": 1,
                "default": null,
                "description": "Print at most this many diagnostics without changing \
                    the exit code. Null (the default) means no limit. Must be at \
                    least 1 in configuration; use the CLI flag to hide all output."
            }
        },
        "definitions": {
            "ruleLevel": {
                "enum": ["off", "on", "info", "warn", "error"],
                "description": "Severity level for a rule: `off` disables it; `on` \
                    reports at its default severity; `info`, `warn`, and `error` set an \
                    explicit severity."
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
        }
    });
    let definitions = schema["definitions"]
        .as_object_mut()
        .expect("schema definitions");
    definitions.insert(
        "ruleWithoutOptions".into(),
        rule_config(json!({
            "type": "object", "additionalProperties": false, "properties": {}
        })),
    );
    for (name, options) in rule_options_schemas() {
        definitions.insert(name.into(), rule_config(options));
    }
    schema
}

/// The schema serialized exactly as it is written to disk: pretty-printed with a
/// trailing newline. Shared by `cargo xtask schema` (writer) and the drift test.
pub fn config_schema_string() -> String {
    let mut s = serde_json::to_string_pretty(&config_schema()).expect("schema serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

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
