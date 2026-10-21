//! Metadata, schema, and legacy configuration contracts.

use std::collections::HashSet;

use falcon_rules::meta::{RULE_METADATA, RuleSource, meta_for};

const RULE_COUNT_DOCUMENTS: &[&str] = &[
    "README.md",
    "docs/getting-started.md",
    "docs/installation.md",
    "docs/configuration.md",
];

fn assert_documented_rule_count(expected: usize, following_words: &[&str]) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for document in RULE_COUNT_DOCUMENTS {
        let contents = std::fs::read_to_string(root.join(document))
            .unwrap_or_else(|error| panic!("{document}: {error}"));
        let words: Vec<_> = contents
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
            .collect();
        let mut found = false;
        for phrase in words.windows(following_words.len() + 1) {
            if phrase[1..] == *following_words
                && let Ok(count) = phrase[0].parse::<usize>()
            {
                assert_eq!(count, expected, "stale rule count in {document}");
                found = true;
            }
        }
        assert!(found, "missing documented rule count in {document}");
    }
}

#[test]
fn recommended_rules_match_documented_count() {
    assert_documented_rule_count(
        RULE_METADATA.iter().filter(|meta| meta.recommended).count(),
        &["recommended"],
    );
}

#[test]
fn total_rules_match_documented_count() {
    assert_documented_rule_count(RULE_METADATA.len(), &["shipped", "rules"]);
}

#[test]
fn every_rule_option_schema_property_has_a_description() {
    fn check(schema: &serde_json::Value, path: &str) {
        match schema {
            serde_json::Value::Object(object) => {
                if let Some(properties) = object
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                {
                    for (name, property) in properties {
                        let path = format!("{path}.{name}");
                        assert!(
                            property
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .is_some_and(|description| !description.trim().is_empty()),
                            "missing option description: {path}"
                        );
                    }
                }
                for (key, value) in object {
                    check(value, &format!("{path}.{key}"));
                }
            }
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    check(value, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }

    for (name, schema) in falcon_rules::schema::rule_options_schemas() {
        check(&schema, name);
    }
}

#[test]
fn every_options_type_is_declared_and_loaded_options_execute() {
    use falcon_analyze::{Workspace, analyze_sequential_collecting_resolving};
    use serde_json::json;

    let schemas = falcon_rules::schema::rule_options_schemas();
    let names: HashSet<_> = schemas.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), schemas.len());
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let option_modules: HashSet<_> = walkdir::WalkDir::new(source_root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "rs")
        })
        .filter(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap()
                .contains("impl_options!(Options)")
        })
        .map(|entry| {
            entry
                .path()
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .replace('_', "-")
        })
        .collect();
    assert_eq!(names, option_modules.iter().map(String::as_str).collect());

    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("falcon.json");
    let mut value = json!({
        "linter": {"rules": {"recommended": false}},
        "cross-file": {"rules": {"recommended": false}}
    });
    for name in names {
        let metadata = meta_for(name).expect("options-bearing rule is registered");
        let section = if metadata.is_cross_file() {
            "cross-file"
        } else {
            "linter"
        };
        let groups = value[section]["rules"].as_object_mut().unwrap();
        let group = groups
            .entry(metadata.group.as_str())
            .or_insert_with(|| json!({}));
        group[name] = json!({"level": "warn", "options": {}});
    }
    std::fs::write(&config_path, serde_json::to_string(&value).unwrap()).unwrap();
    let loaded = falcon_rules::load_config(&config_path).unwrap();
    let registry = falcon_rules::build_registry(&loaded.config);
    let files = [(
        directory.path().join("main.dart"),
        "class Probe { int value = 1; }\nvoid main() { print(1); }\n".to_owned(),
    )];
    let (diagnostics, files, indexes) = analyze_sequential_collecting_resolving(
        &registry,
        &files,
        &loaded.config,
        true,
        true,
        Workspace::default(),
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != "config")
    );
    let registry = falcon_rules::build_cross_file_registry(&loaded.config);
    let diagnostics =
        falcon_rules::run_cross_file(&registry, &files, &loaded.config, indexes.as_ref()).unwrap();
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != "config")
    );
}

#[test]
fn invalid_unloaded_options_report_config_errors_instead_of_panicking() {
    use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
    use serde_json::json;

    let config = serde_json::from_value(json!({
        "linter": {"rules": {"complexity": {"max-lines-for-file": {
            "level": "warn", "options": {"max-lines": "bad"}
        }}}}
    }))
    .unwrap();
    let source = "void main() {}";
    let parsed = std::sync::Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parser::parse(source).tree,
    });
    let path = std::path::Path::new("main.dart");
    let context = AnalyzeContext::new(path, &config, &parsed);
    let falcon_rules::meta::RuleKind::File { make, .. } =
        meta_for("max-lines-for-file").unwrap().kind
    else {
        unreachable!()
    };
    let diagnostics = make().analyze("max-lines-for-file", parsed.program(), &context);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "config");
    assert!(
        diagnostics[0]
            .message
            .contains("linter.rules.complexity.max-lines-for-file.options.max-lines")
    );
    let registry = falcon_rules::build_registry(&config);
    let diagnostics = registry.run_all(parsed.program(), &context);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "config");
}

#[test]
fn repeated_lazy_option_errors_are_reported_once_with_their_origin() {
    use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
    use serde_json::json;

    let source = concat!(
        "void first(bool flag) { if (flag) {} }\n",
        "void second(bool flag) { if (flag) {} }\n",
        "void third(bool flag) { if (flag) {} }\n",
        "void fourth(bool flag) { if (flag) {} }\n",
        "void fifth(bool flag) { if (flag) {} }\n",
    );
    let parsed = std::sync::Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parser::parse(source).tree,
    });
    let path = std::path::Path::new("lib/main.dart");
    let falcon_rules::meta::RuleKind::File { make, .. } =
        meta_for("cyclomatic-complexity").unwrap().kind
    else {
        unreachable!()
    };
    for in_override in [false, true] {
        let mut value = json!({
            "linter": {"rules": {
                "recommended": false,
                "complexity": {"cyclomatic-complexity": {
                    "level": "warn", "options": {"max-complexity": 0}
                }}
            }}
        });
        let invalid = json!({
            "level": "warn", "options": {"max-complexity": "bad"}
        });
        if in_override {
            value["overrides"] = json!([{
                "includes": ["lib/**"],
                "linter": {"rules": {"complexity": {"cyclomatic-complexity": invalid}}}
            }]);
        } else {
            value["linter"]["rules"]["complexity"]["cyclomatic-complexity"] = invalid;
        }
        let config = serde_json::from_value(value).unwrap();
        let prefix = if in_override { "overrides[0]." } else { "" };
        let expected = format!(
            "`{prefix}linter.rules.complexity.cyclomatic-complexity.options.max-complexity`"
        );
        for direct in [true, false] {
            let context = AnalyzeContext::new(path, &config, &parsed);
            let diagnostics = if direct {
                make().analyze("cyclomatic-complexity", parsed.program(), &context)
            } else {
                falcon_rules::build_registry(&config).run_all(parsed.program(), &context)
            };
            assert_eq!(
                diagnostics.len(),
                1,
                "{in_override}, {direct}: {diagnostics:?}"
            );
            let diagnostic = &diagnostics[0];
            assert_eq!(diagnostic.rule, "config");
            assert_eq!(diagnostic.severity, falcon_diagnostics::Severity::Error);
            assert!(diagnostic.message.contains(&expected), "{diagnostic:?}");
            assert_eq!(diagnostic.file_path, "lib/main.dart");
        }
    }
}

#[test]
fn cross_file_lazy_option_errors_report_base_and_override_origins() {
    use falcon_analyze::{ParsedFile, ProjectFile};
    use serde_json::json;

    let source = "import 'dart:async';\nvoid main() {}\n";
    let files = [ProjectFile {
        path: "lib/main.dart".into(),
        parsed: std::sync::Arc::new(ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parser::parse(source).tree,
        }),
        has_parse_errors: false,
    }];
    for in_override in [false, true] {
        let mut value = json!({
            "cross-file": {"rules": {
                "recommended": false,
                "correctness": {"banned-imports": {
                    "level": "warn", "options": {"rules": []}
                }}
            }}
        });
        let invalid = json!({
            "level": "warn",
            "options": {"rules": [{
                "files": ["lib/**"], "deny": ["dart:async", 42], "message": "Avoid async."
            }]}
        });
        if in_override {
            value["overrides"] = json!([{
                "includes": ["lib/**"],
                "cross-file": {"rules": {"correctness": {"banned-imports": invalid}}}
            }]);
        } else {
            value["cross-file"]["rules"]["correctness"]["banned-imports"] = invalid;
        }
        let config = serde_json::from_value(value).unwrap();
        let registry = falcon_rules::build_cross_file_registry(&config);
        let diagnostics = falcon_rules::run_cross_file(&registry, &files, &config, None).unwrap();
        assert_eq!(diagnostics.len(), 1, "{in_override}: {diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule, "config");
        assert_eq!(diagnostic.severity, falcon_diagnostics::Severity::Error);
        assert_eq!(diagnostic.file_path, "lib/main.dart");
        assert_eq!(
            diagnostic.span,
            falcon_diagnostics::Span { start: 0, end: 0 }
        );
        let prefix = if in_override { "overrides[0]." } else { "" };
        let expected = format!(
            "`{prefix}cross-file.rules.correctness.banned-imports.options.rules[0].deny[1]`"
        );
        assert!(diagnostic.message.contains(&expected), "{diagnostic:?}");
    }
}

#[test]
fn rule_sections_reference_shared_options_definitions() {
    let schema = falcon_rules::schema::config_schema();
    let exceptions = &schema["definitions"]["prefer-correct-identifier-length"]["oneOf"][1]["properties"]
        ["options"]["properties"]["exceptions"];
    assert_eq!(exceptions["type"], "array");
    assert!(exceptions.get("uniqueItems").is_none());
    let names: HashSet<_> = falcon_rules::schema::rule_options_schemas()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for metadata in RULE_METADATA {
        let definition = if names.contains(metadata.name) {
            metadata.name
        } else {
            "ruleWithoutOptions"
        };
        let expected = format!("#/definitions/{definition}");
        let section = if metadata.is_cross_file() {
            "cross-file"
        } else {
            "linter"
        };
        assert_eq!(
            schema["properties"][section]["properties"]["rules"]["properties"]
                [metadata.group.as_str()]["properties"][metadata.name]["$ref"],
            expected
        );
        let section = if metadata.is_cross_file() {
            "overrideCrossFile"
        } else {
            "overrideLinter"
        };
        assert_eq!(
            schema["definitions"][section]["properties"]["rules"]["properties"]
                [metadata.group.as_str()]["properties"][metadata.name]["$ref"],
            expected
        );
        assert!(schema["definitions"].get(definition).is_some());
    }
}

#[test]
fn rule_named_changesets_mention_rule_ids() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.changeset");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("cannot read {}: {error}", directory.display()),
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let filename = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .replace('_', "-");
        let contents = std::fs::read_to_string(&path).unwrap();
        let body = contents
            .strip_prefix("---\n")
            .and_then(|contents| contents.split_once("\n---"))
            .map_or(contents.as_str(), |(_, body)| body);
        for rule in RULE_METADATA {
            if filename.match_indices(rule.name).any(|(start, _)| {
                (start == 0 || filename.as_bytes()[start - 1] == b'-')
                    && (start + rule.name.len() == filename.len()
                        || filename.as_bytes()[start + rule.name.len()] == b'-')
            }) {
                assert!(
                    body.contains(rule.name),
                    "{} must mention rule id `{}`",
                    path.display(),
                    rule.name
                );
            }
        }
    }
}

#[test]
fn registry_resolution_requirement_uses_registered_names() {
    use falcon_analyze::RuleRegistry;
    use falcon_rules::meta::RuleKind;
    let mut registry = RuleRegistry::new();
    assert!(!falcon_rules::requires_resolution(&registry));
    for (name, expected) in [
        ("avoid-dynamic", false),
        ("avoid-ignoring-return-values", true),
    ] {
        let RuleKind::File { make, .. } = meta_for(name).unwrap().kind else {
            panic!("expected file rule");
        };
        registry.register(name, make());
        assert_eq!(falcon_rules::requires_resolution(&registry), expected);
    }
    assert!(!falcon_rules::rule_requires_resolution("unknown-rule"));
}

#[test]
fn domain_metadata_preserves_configuration_names() {
    for meta in RULE_METADATA {
        let expected: Vec<_> = meta.domain.iter().map(|domain| domain.as_str()).collect();
        assert_eq!(meta.domain_names(), expected);
    }
}

#[test]
fn rule_groups_have_stable_order_and_configuration_names() {
    use falcon_rules::meta::RuleGroup;

    let groups: Vec<_> = RuleGroup::ALL.iter().map(|group| group.as_str()).collect();
    assert_eq!(
        groups,
        [
            "a11y",
            "complexity",
            "correctness",
            "performance",
            "security",
            "style",
            "suspicious",
        ]
    );
}

#[test]
fn schema_exposes_all_groups_in_both_sections_including_empty_groups() {
    use falcon_rules::meta::RuleGroup;

    let schema = falcon_rules::schema::config_schema();
    for section in ["linter", "cross-file"] {
        let rules = &schema["properties"][section]["properties"]["rules"];
        assert_eq!(rules["additionalProperties"], false);
        assert_eq!(
            rules["properties"].as_object().unwrap().len(),
            RuleGroup::ALL.len() + 1,
            "all groups plus the recommended toggle in {section}"
        );
        for group in RuleGroup::ALL {
            let group_schema = &rules["properties"][group.as_str()];
            assert_eq!(group_schema["type"], "object");
            assert_eq!(group_schema["additionalProperties"], false);
            assert!(group_schema["properties"].is_object());
            assert!(group_schema.get("required").is_none());
            assert!(group_schema.get("minProperties").is_none());
        }
    }
    assert!(
        schema["properties"]["cross-file"]["properties"]["rules"]["properties"]["a11y"]
            ["properties"]
            .as_object()
            .unwrap()
            .is_empty()
    );

    let _: falcon_config::FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": { "rules": { "a11y": {}, "security": {} } },
        "cross-file": { "rules": { "a11y": {}, "security": {} } }
    }))
    .expect("empty reserved groups are valid configuration");
}

#[test]
fn secure_pubspec_urls_is_only_exposed_under_cross_file_security() {
    use falcon_rules::meta::RuleGroup;

    let meta = meta_for("secure-pubspec-urls").unwrap();
    assert_eq!(meta.group, RuleGroup::Security);
    assert!(meta.is_cross_file());

    let schema = falcon_rules::schema::config_schema();
    for section in ["linter", "cross-file"] {
        for group in RuleGroup::ALL {
            let properties = &schema["properties"][section]["properties"]["rules"]["properties"]
                [group.as_str()]["properties"];
            assert_eq!(
                properties.get(meta.name).is_some(),
                section == "cross-file" && *group == RuleGroup::Security,
                "{section}.rules.{}",
                group.as_str()
            );
        }
    }
}

#[test]
fn committed_schema_matches_generator() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/falcon.schema.json"
    );
    let committed = std::fs::read_to_string(path)
        .expect("schema/falcon.schema.json exists — run `cargo xtask schema`");
    assert_eq!(
        committed,
        falcon_rules::schema::config_schema_string(),
        "schema/falcon.schema.json is stale; run `cargo xtask schema`"
    );
}

#[test]
fn metadata_names_are_unique() {
    let mut seen = HashSet::new();
    for meta in RULE_METADATA {
        assert!(
            seen.insert(meta.name),
            "duplicate metadata entry: {}",
            meta.name
        );
    }
}

#[test]
fn aliases_do_not_shadow_live_rule_names() {
    let names: HashSet<_> = RULE_METADATA.iter().map(|meta| meta.name).collect();
    for (alias, _) in falcon_rules::meta::RULE_ALIASES {
        assert!(!names.contains(alias), "alias shadows live rule: {alias}");
    }
}

#[test]
fn every_rule_has_a_source_with_a_non_empty_upstream_id() {
    for meta in RULE_METADATA {
        match meta.source {
            RuleSource::DartCodeLinter(id)
            | RuleSource::PyramidLint(id)
            | RuleSource::Lints(id) => assert!(
                !id.is_empty(),
                "rule `{}` has a ported source with an empty upstream id",
                meta.name
            ),
            RuleSource::Falcon => {}
        }
    }
}

#[test]
fn meta_for_finds_known_and_rejects_unknown() {
    assert!(meta_for("avoid-dynamic").is_some());
    assert!(meta_for("this-rule-does-not-exist").is_none());
}

#[test]
fn legacy_aliases_resolve_to_canonical_metadata() {
    use falcon_rules::meta::{RULE_ALIASES, canonical_rule_name};

    // Every alias maps to a real canonical rule, and its canonical id is not
    // itself an alias key (no chains).
    for (old, canonical) in RULE_ALIASES {
        assert_eq!(canonical_rule_name(old), *canonical, "alias {old}");
        assert!(
            meta_for(old).is_some_and(|m| m.name == *canonical),
            "alias `{old}` must resolve to `{canonical}`"
        );
        assert!(
            !RULE_ALIASES.iter().any(|(k, _)| k == canonical),
            "canonical id `{canonical}` must not also be an alias key"
        );
    }
}

#[test]
fn canonicalize_config_rewrites_legacy_rule_keys() {
    use falcon_config::FalconConfig;
    use falcon_rules::meta::canonicalize_config;

    // A pre-1.0 config using a snake_case id and a removed twin id still turns
    // the canonical rules on after canonicalization.
    let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": {
            "rules": {
                "complexity": { "max_lines_for_file": "error" },
                "style": { "no_magic_number": "warn" }
            }
        }
    }))
    .expect("valid config");

    let warnings = canonicalize_config(&mut config);
    assert_eq!(warnings.len(), 2);

    assert_eq!(
        config.resolve_rule("complexity", "max-lines-for-file", false, &[]),
        Some(falcon_config::ResolvedSeverity::Error),
        "legacy `max_lines_for_file` key must enable `max-lines-for-file`"
    );
    assert_eq!(
        config.resolve_rule("style", "no-magic-number", false, &[]),
        Some(falcon_config::ResolvedSeverity::Warn),
        "removed twin `no_magic_number` key must enable `no-magic-number`"
    );
}

#[test]
fn canonicalization_preserves_collisions_unknown_keys_and_all_override_maps() {
    use falcon_config::{FalconConfig, RulePlainConfiguration};
    use falcon_rules::meta::canonicalize_config;

    for (first, second) in [("error", "warn"), ("off", "error")] {
        let rules = serde_json::json!({
            "suspicious": {
                "avoid_empty_blocks": first,
                "no_empty_block": second,
                "no-empty-block": "info",
                "unknown-rule": "off"
            }
        });
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": { "rules": rules },
            "cross-file": { "rules": rules },
            "overrides": [{
                "includes": ["test/**"],
                "linter": { "rules": rules },
                "cross-file": { "rules": rules }
            }]
        }))
        .expect("valid config");
        let warnings = canonicalize_config(&mut config);
        assert_eq!(warnings.len(), 16);
        let once = serde_json::to_value(&config).expect("serializable config");
        assert!(canonicalize_config(&mut config).is_empty());
        assert_eq!(serde_json::to_value(&config).unwrap(), once);
        for rules in [
            &config.linter.rules,
            &config.cross_file.rules,
            &config.overrides[0].linter.as_ref().unwrap().rules,
            &config.overrides[0].cross_file.as_ref().unwrap().rules,
        ] {
            let group = &rules.groups["suspicious"];
            assert_eq!(group.len(), 2);
            assert_eq!(
                group["no-empty-block"].level(),
                RulePlainConfiguration::Error
            );
            assert_eq!(group["unknown-rule"].level(), RulePlainConfiguration::Off);
        }
    }
}

#[test]
fn equal_severity_collisions_preserve_canonical_options_then_first_legacy_options() {
    use falcon_config::FalconConfig;
    use falcon_rules::meta::canonicalize_config;

    for canonical in [false, true] {
        let first = serde_json::json!({ "level": "warn", "options": { "winner": "first" } });
        let canonical_config =
            serde_json::json!({ "level": "on", "options": { "winner": "canonical" } });
        let mut group = serde_json::json!({
            "avoid_empty_blocks": first,
            "no_empty_block": { "level": "warn", "options": { "winner": "second" } }
        });
        if canonical {
            group["no-empty-block"] = canonical_config.clone();
        }
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": { "rules": { "suspicious": group } }
        }))
        .unwrap();
        let _ = canonicalize_config(&mut config);
        let result = &config.linter.rules.groups["suspicious"];
        assert_eq!(result.len(), 1);
        assert_eq!(
            serde_json::to_value(&result["no-empty-block"]).unwrap(),
            if canonical { canonical_config } else { first }
        );
    }
}

#[test]
fn registered_file_rules_choose_shared_collectors_or_whole_file_checks() {
    use std::collections::BTreeSet;

    use falcon_analyze::{
        AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, RulePreparation,
        SemanticModel, SignatureIndex, TypeIndex, group_libraries, library_unit,
    };
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use falcon_rules::meta::RuleKind;

    let whole_file_checks = [
        ("always-use-package-imports", "Import URI schemes."),
        (
            "avoid-abbreviations-in-doc-comments",
            "Raw documentation comment lines.",
        ),
        ("avoid-banned-exports", "Export directive URI matches."),
        (
            "avoid-barrel-files",
            "Whole-file exports without declarations.",
        ),
        (
            "avoid-double-slash-imports",
            "Import directive URI path segments.",
        ),
        (
            "avoid-duplicate-exports",
            "Export URI and visibility comparison.",
        ),
        (
            "avoid-duplicate-named-imports",
            "Import URI and prefix comparison.",
        ),
        (
            "avoid-duplicate-test-assertions",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "avoid-commented-out-code",
            "Raw non-documentation comment tokens.",
        ),
        (
            "avoid-empty-test-groups",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "avoid-top-level-members-in-tests",
            "Test filename eligibility before structural collection.",
        ),
        ("avoid-relative-lib-imports", "Import URI path segments."),
        (
            "avoid-web-libraries-in-flutter",
            "Import URIs under Flutter package conditions.",
        ),
        ("combinators-ordering", "Import combinator ordering."),
        ("comment-references", "Documentation reference blocks."),
        (
            "conditional-uri-does-not-exist",
            "Conditional URI filesystem resolution.",
        ),
        (
            "dangling-library-doc-comments",
            "Documentation token attachment and source trivia.",
        ),
        ("directives-ordering", "Ordering across directive lists."),
        ("eol-at-end-of-file", "Whole-source line-ending suffix."),
        ("file-names", "File path basename."),
        ("flutter-style-todos", "Comment token text."),
        (
            "format-comment",
            "Source comment grouping and sentence formatting.",
        ),
        ("implementation-imports", "Import URIs and package layout."),
        ("library-names", "Library directive name."),
        ("library-prefixes", "Import directive prefixes."),
        ("max-lines-for-file", "Whole-source line count."),
        (
            "missing-test-assertion",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "no-leading-underscores-for-library-prefixes",
            "Import directive prefixes.",
        ),
        (
            "prefer-correct-test-file-name",
            "Package test directory and filename eligibility before structural collection.",
        ),
        (
            "prefer-expect-later",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "prefer-test-matchers",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "prefer-unique-test-names",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "prefer-commenting-analyzer-ignores",
            "Analyzer ignore comment tokens and adjacent explanation trivia.",
        ),
        (
            "prefer-relative-imports",
            "Import URIs and package directory context.",
        ),
        (
            "slash-for-doc-comments",
            "Documentation comment token spelling.",
        ),
        (
            "unintended-html-in-doc-comment",
            "Documentation source text and fences.",
        ),
        (
            "unnecessary-library-directive",
            "Library directives and preceding documentation trivia.",
        ),
        (
            "unnecessary-library-name",
            "Optional library directive name.",
        ),
        (
            "use-string-in-part-of-directives",
            "Part-of directive form.",
        ),
    ];
    let expected: BTreeSet<_> = whole_file_checks
        .into_iter()
        .map(|(name, reason)| {
            assert!(!reason.is_empty());
            name
        })
        .collect();
    let mut diagnostics_only: BTreeSet<_> = RULE_METADATA
        .iter()
        .filter(|metadata| matches!(metadata.kind, RuleKind::File { .. }))
        .map(|metadata| metadata.name)
        .collect();
    // Exercise package-, option-, and candidate-gated collectors rather than
    // treating an ineligible probe's empty diagnostics as a whole-file check.
    let package = tempfile::tempdir().unwrap();
    std::fs::write(
        package.path().join("pubspec.yaml"),
        "name: collector_probe\n",
    )
    .unwrap();
    std::fs::create_dir(package.path().join("lib")).unwrap();
    let path = package.path().join("lib/structural_collectors.dart");
    let config: FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": {
            "rules": {
                "style": {
                    "enum-constants-ordering": { "level": "warn", "options": { "order": "ascending" } },
                    "map-keys-ordering": { "level": "warn", "options": { "order": "ascending" } },
                    "parameters-ordering": { "level": "warn", "options": { "order": "ascending" } },
                    "pattern-fields-ordering": { "level": "warn", "options": { "order": "ascending" } },
                    "record-fields-ordering": { "level": "warn", "options": { "order": "ascending" } },
                    "extend-equatable": {
                        "level": "warn",
                        "options": { "class-name-pattern": "Probe$" }
                    },
                    "use-design-system-item": {
                        "level": "warn",
                        "options": { "items": [{ "class-name": "Container" }] }
                    },
                    "prefer-correct-error-name": {
                        "level": "warn",
                        "options": { "allowed-names": ["error"] }
                    },
                    "prefer-correct-handler-name": {
                        "level": "warn",
                        "options": { "name-pattern": "^handle[A-Z]" }
                    },
                    "prefer-correct-callback-field-name": {
                        "level": "warn",
                        "options": { "name-pattern": "^on[A-Z]" }
                    }
                },
                "suspicious": {
                    "avoid-banned-names": {
                        "level": "warn",
                        "options": { "rules": [{ "deny": ["Probe"], "message": "Use another name." }] }
                    },
                    "avoid-banned-types": {
                        "level": "warn",
                        "options": { "rules": [{ "deny": ["Probe"], "message": "Use another type." }] }
                    },
                    "avoid-banned-annotations": {
                        "level": "warn",
                        "options": { "rules": [{ "deny": ["override"], "message": "Use another annotation." }] }
                    },
                    "banned-usage": {
                        "level": "warn",
                        "options": { "rules": [{ "deny": ["print"], "message": "Use another function." }] }
                    }
                }
            }
        }
    }))
    .unwrap();
    let source = r#"
typedef _Callback = void Function();
int? _deferred;
class Probe {
  static const Probe first = Probe._();
  static const Probe second = Probe._();
  const Probe._();
}
class InitializingFormalProbe {
  final int? value;
  InitializingFormalProbe(this.value) : assert(value != null);
  void method(int parameter) { parameter = 1; }
}
void main() {
  _deferred = 1;
  print(_deferred!);
  const bool.fromEnvironment('COLLECTOR_PROBE');
}
// This deliberately long source line makes the line-length collector eligible for its shared callback traversal.
"#;
    for (language_override, part_directive) in [
        ("", ""),
        ("// @dart = 2.19\n", ""),
        ("", "part 'probe_part.dart';\n"),
        ("", "part of 'probe_owner.dart';\n"),
    ] {
        let source = format!("{language_override}{part_directive}{source}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty());
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into_boxed_str(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let (sibling_name, sibling_source) = if part_directive.is_empty() {
            ("probe_part.dart", "")
        } else if program.part_of_directive().is_some() {
            ("probe_owner.dart", "part 'structural_collectors.dart';")
        } else {
            ("probe_part.dart", "part of 'structural_collectors.dart';")
        };
        let sibling_path = path.with_file_name(sibling_name);
        let sibling = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: sibling_source.into(),
            tree: parse(sibling_source).tree,
        });
        let grouping =
            group_libraries(&[(path.clone(), program), (sibling_path, sibling.program())]);
        let unit = library_unit(&grouping, &[&parsed, &sibling], 0);
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
        let signatures = SignatureIndex::from_program(program, &model);
        let context = AnalyzeContext::new(&path, &config, &parsed)
            .with_types(&types)
            .with_identities(&identities)
            .with_signatures(&signatures)
            .with_library(&unit);
        let unresolved_context = AnalyzeContext::new(&path, &config, &parsed);
        for metadata in RULE_METADATA {
            let RuleKind::File {
                make,
                needs_resolution,
            } = metadata.kind
            else {
                continue;
            };
            let preparation = make().prepare(program, &context);
            if !needs_resolution {
                assert_eq!(
                    std::mem::discriminant(&make().prepare(program, &unresolved_context)),
                    std::mem::discriminant(&preparation),
                    "{} changes preparation kind without resolution but is not declared resolved",
                    metadata.name
                );
            }
            match preparation {
                RulePreparation::Structural(collector) => {
                    assert!(
                        collector.has_derived_interests(),
                        "{} must derive callback interests with structural_visitor!",
                        metadata.name
                    );
                    if part_directive.is_empty() {
                        diagnostics_only.remove(metadata.name);
                    }
                }
                RulePreparation::Semantic(_) => {
                    assert!(
                        needs_resolution,
                        "{} returns a semantic collector but is not declared resolved",
                        metadata.name
                    );
                    if part_directive.is_empty() {
                        diagnostics_only.remove(metadata.name);
                    }
                }
                RulePreparation::Diagnostics(_) => {}
            }
        }
    }
    assert_eq!(diagnostics_only, expected);
}

#[test]
fn library_dependent_rules_analyze_parts_when_enabled_alone() {
    use falcon_analyze::{RuleRegistry, Workspace, analyze_sequential_collecting_resolving};
    use falcon_config::FalconConfig;
    use falcon_rules::meta::RuleKind;

    for (name, owner, part, expected) in [
        (
            "avoid-duplicate-mixins",
            include_str!("corpus/avoid-duplicate-mixins/project/owner.dart"),
            include_str!("corpus/avoid-duplicate-mixins/project/part.dart"),
            2,
        ),
        (
            "avoid-private-typedef-functions",
            include_str!("corpus/avoid-private-typedef-functions/project/owner.dart"),
            include_str!("corpus/avoid-private-typedef-functions/project/part.dart"),
            1,
        ),
    ] {
        let metadata = RULE_METADATA
            .iter()
            .find(|metadata| metadata.name == name)
            .unwrap();
        let RuleKind::File { make, .. } = metadata.kind else {
            panic!("{name} must be a per-file rule");
        };
        let mut registry = RuleRegistry::with_lookup(falcon_rules::meta::suppression_lookup);
        registry.register(name, make());
        let files = [
            (
                std::path::PathBuf::from("/project/lib/owner.dart"),
                owner.to_owned(),
            ),
            (
                std::path::PathBuf::from("/project/lib/part.dart"),
                part.to_owned(),
            ),
        ];
        let (diagnostics, _, _) = analyze_sequential_collecting_resolving(
            &registry,
            &files,
            &FalconConfig::default(),
            false,
            falcon_rules::requires_resolution(&registry),
            Workspace::default(),
        );
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rule)
                .collect::<Vec<_>>(),
            vec![name; expected],
            "{name} must see sibling declarations when enabled alone"
        );
    }
}

#[test]
fn typed_and_json_canonicalization_share_severity_options_and_tie_policy() {
    use falcon_config::FalconConfig;
    use falcon_rules::meta::{canonicalize_config, canonicalize_rule_map};

    for alias_level in ["off", "info", "on", "warn", "error"] {
        for canonical_level in ["off", "info", "on", "warn", "error"] {
            for alias_first in [true, false] {
                let alias = serde_json::json!({
                    "level": alias_level, "options": { "source": "alias" }
                });
                let canonical = serde_json::json!({
                    "level": canonical_level, "options": { "source": "canonical" }
                });
                let mut entries = serde_json::Map::new();
                let pairs = if alias_first {
                    [("no_empty_block", alias), ("no-empty-block", canonical)]
                } else {
                    [("no-empty-block", canonical), ("no_empty_block", alias)]
                };
                entries.extend(
                    pairs
                        .into_iter()
                        .map(|(key, value)| (key.to_owned(), value)),
                );
                let mut typed: FalconConfig = serde_json::from_value(serde_json::json!({
                    "linter": { "rules": { "suspicious": entries } }
                }))
                .unwrap();
                let json_warnings = canonicalize_rule_map(&mut entries, "linter.rules.suspicious");
                let typed_warnings = canonicalize_config(&mut typed);
                assert_eq!(json_warnings.len(), 2);
                assert_eq!(typed_warnings.len(), 2);
                assert!(
                    json_warnings
                        .iter()
                        .any(|warning| warning.contains("discarded") && warning.contains("source"))
                );
                assert!(
                    typed_warnings
                        .iter()
                        .any(|warning| warning.contains("discarded") && warning.contains("source"))
                );
                assert_eq!(
                    serde_json::to_value(&typed.linter.rules.groups["suspicious"]).unwrap(),
                    serde_json::Value::Object(entries.clone()),
                    "{alias_level}/{canonical_level}, alias first: {alias_first}"
                );
                assert!(canonicalize_rule_map(&mut entries, "linter.rules.suspicious").is_empty());
            }
        }
    }
}
