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
        let entry = groups.entry(meta.group.to_string()).or_insert_with(|| {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {},
            })
        });
        if entry.get("description").is_none() {
            let desc = format!("Per-rule severity levels for the `{}` group.", meta.group);
            entry
                .as_object_mut()
                .expect("group object")
                .insert("description".into(), json!(desc));
        }
        entry["properties"]
            .as_object_mut()
            .expect("properties object")
            .insert(
                meta.name.to_string(),
                json!({ "$ref": "#/definitions/ruleConfig" }),
            );
    }
    let mut props = Map::new();
    props.insert(
        "recommended".into(),
        json!({
            "type": "boolean",
            "default": true,
            "description": "Enable the recommended preset for this section. \
                When on, every rule in the preset reports at its default severity \
                unless individually overridden. Defaults to true.",
        }),
    );
    for (group, schema) in groups {
        props.insert(group, schema);
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Rule severity levels grouped by category, plus the \
            `recommended` preset toggle.",
        "properties": Value::Object(props),
    })
}

/// Build the complete JSON Schema (draft-07) for falcon.json.
pub fn config_schema() -> Value {
    let mut domain_props = Map::new();
    for domain in DOMAINS {
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

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": SCHEMA_URL,
        "title": "falcon.json",
        "description": "Configuration for the falcon Dart linter.",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "$schema": {
                "type": "string",
                "description": "URL of the falcon.json JSON Schema, enabling rule \
                    autocomplete and level validation in editors."
            },
            "files": {
                "type": "object",
                "additionalProperties": false,
                "description": "Which files falcon discovers and analyzes.",
                "properties": {
                    "includes": string_globs(
                        "Glob patterns for the files falcon analyzes. When omitted, \
                         every Dart file under the working directory is analyzed."
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
                        "type": "object",
                        "additionalProperties": false,
                        "description": "Per-domain gating for domain-scoped rules \
                            (for example Flutter rules).",
                        "properties": Value::Object(domain_props)
                    },
                    "rules": section_rules(false)
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
                    "rules": section_rules(true)
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
                            "Glob patterns selecting the files this override applies to."
                        ),
                        "linter": { "$ref": "#/definitions/overrideLinter" },
                        "cross-file": { "$ref": "#/definitions/overrideCrossFile" }
                    }
                }
            },
            "max-errors": {
                "type": ["integer", "null"],
                "minimum": 1,
                "default": null,
                "description": "Stop after emitting this many diagnostics. Null (the \
                    default) means no limit. Must be at least 1; 0 is rejected because \
                    it would suppress every diagnostic."
            }
        },
        "definitions": {
            "ruleLevel": {
                "enum": ["off", "on", "info", "warn", "error"],
                "description": "Severity level for a rule: `off` disables it; `on` \
                    reports at its default severity; `info`, `warn`, and `error` set an \
                    explicit severity."
            },
            "ruleConfig": {
                "description": "A rule's configuration: either a bare severity level, or \
                    an object with a `level` and rule-specific `options`.",
                "oneOf": [
                    { "$ref": "#/definitions/ruleLevel" },
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["level"],
                        "properties": {
                            "level": { "$ref": "#/definitions/ruleLevel" },
                            "options": {
                                "type": "object",
                                "description": "Rule-specific options (see the rule's \
                                    documentation for supported keys)."
                            }
                        }
                    }
                ]
            },
            "overrideLinter": {
                "type": "object",
                "additionalProperties": false,
                "description": "Linter settings applied by an override to its matched files.",
                "properties": {
                    "enabled": {
                        "type": "boolean",
                        "description": "Master switch for the single-file linter within \
                            this override."
                    },
                    "rules": section_rules(false)
                }
            },
            "overrideCrossFile": {
                "type": "object",
                "additionalProperties": false,
                "description": "Cross-file settings applied by an override to its matched files.",
                "properties": {
                    "enabled": {
                        "type": "boolean",
                        "description": "Master switch for cross-file analysis within this \
                            override."
                    },
                    "rules": section_rules(true)
                }
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
