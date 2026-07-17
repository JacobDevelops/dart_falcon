//! JSON Schema for falcon.json, derived from the rule metadata table.
//!
//! [`config_schema`] builds a draft-07 schema whose rule-name enums come
//! straight from [`RULE_METADATA`](crate::meta::RULE_METADATA), so editors that
//! resolve the config's `$schema` URL get rule autocomplete and level validation
//! for free. `cargo xtask schema` writes it to `schema/falcon.schema.json`; the
//! committed file's drift from this generator is guarded by `tests/meta_tests.rs`.

use serde_json::{Map, Value, json};

use crate::meta::{DOMAINS, RULE_METADATA};

/// Canonical published location of the schema — the value users point `$schema`
/// at, and the schema's own `$id`. Served raw from the default branch so it
/// tracks the rule set without a release cut.
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json";

/// The `rules` sub-schema for one section: `recommended` plus one object per
/// group listing exactly that section's rules (file rules unless `cross_file`).
fn section_rules(cross_file: bool) -> Value {
    let mut groups: Map<String, Value> = Map::new();
    for meta in RULE_METADATA {
        if meta.cross_file != cross_file {
            continue;
        }
        let entry = groups.entry(meta.group.to_string()).or_insert_with(
            || json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        );
        entry["properties"]
            .as_object_mut()
            .expect("properties object")
            .insert(
                meta.name.to_string(),
                json!({ "$ref": "#/definitions/ruleConfig" }),
            );
    }
    let mut props = Map::new();
    props.insert("recommended".into(), json!({ "type": "boolean" }));
    for (group, schema) in groups {
        props.insert(group, schema);
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": Value::Object(props),
    })
}

/// Build the complete JSON Schema (draft-07) for falcon.json.
pub fn config_schema() -> Value {
    let mut domain_props = Map::new();
    for domain in DOMAINS {
        domain_props.insert(
            domain.to_string(),
            json!({ "enum": ["all", "recommended", "none"] }),
        );
    }
    let string_globs = json!({ "type": "array", "items": { "type": "string" } });

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": SCHEMA_URL,
        "title": "falcon.json",
        "description": "Configuration for the falcon Dart linter.",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "$schema": { "type": "string" },
            "files": {
                "type": "object",
                "additionalProperties": false,
                "properties": { "includes": string_globs }
            },
            "linter": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "domains": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": Value::Object(domain_props)
                    },
                    "rules": section_rules(false)
                }
            },
            "cross-file": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "rules": section_rules(true)
                }
            },
            "overrides": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "includes": string_globs,
                        "linter": { "$ref": "#/definitions/overrideLinter" },
                        "cross-file": { "$ref": "#/definitions/overrideCrossFile" }
                    }
                }
            },
            "max-errors": { "type": ["integer", "null"], "minimum": 0 }
        },
        "definitions": {
            "ruleLevel": { "enum": ["off", "on", "info", "warn", "error"] },
            "ruleConfig": {
                "oneOf": [
                    { "$ref": "#/definitions/ruleLevel" },
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["level"],
                        "properties": {
                            "level": { "$ref": "#/definitions/ruleLevel" },
                            "options": { "type": "object" }
                        }
                    }
                ]
            },
            "overrideLinter": {
                "type": "object",
                "additionalProperties": false,
                "properties": { "enabled": { "type": "boolean" }, "rules": section_rules(false) }
            },
            "overrideCrossFile": {
                "type": "object",
                "additionalProperties": false,
                "properties": { "enabled": { "type": "boolean" }, "rules": section_rules(true) }
            }
        }
    })
}

/// The schema serialized exactly as it is written to disk: pretty-printed with a
/// trailing newline. Shared by `cargo xtask schema` (writer) and the drift test.
pub fn config_schema_string() -> String {
    let mut s = serde_json::to_string_pretty(&config_schema()).expect("schema serializes");
    s.push('\n');
    s
}
