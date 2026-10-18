//! Metadata, schema, and legacy configuration contracts.

use std::collections::HashSet;

use falcon_rules::meta::{RULE_METADATA, RuleSource, meta_for};

#[test]
fn recommended_rules_match_documented_count() {
    assert_eq!(RULE_METADATA.len(), 368);
    assert_eq!(
        RULE_METADATA.iter().filter(|meta| meta.recommended).count(),
        207
    );
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

    canonicalize_config(&mut config);

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
        canonicalize_config(&mut config);
        let once = serde_json::to_value(&config).expect("serializable config");
        canonicalize_config(&mut config);
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
        canonicalize_config(&mut config);
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
        SemanticModel, SignatureIndex, TypeIndex,
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
        (
            "avoid-duplicate-test-assertions",
            "Test filename and framework import eligibility before semantic collection.",
        ),
        (
            "avoid-commented-out-code",
            "Raw non-documentation comment tokens.",
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
                    "use-design-system-item": {
                        "level": "warn",
                        "options": { "items": [{ "class_name": "Container" }] }
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
    for language_override in ["", "// @dart = 2.19\n"] {
        let source = format!("{language_override}{source}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty());
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into_boxed_str(),
            tree: parsed.tree,
        });
        let program = parsed.program();
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
            .with_signatures(&signatures);
        for metadata in RULE_METADATA {
            let RuleKind::File { make, .. } = metadata.kind else {
                continue;
            };
            match make().prepare(program, &context) {
                RulePreparation::Structural(collector) => {
                    assert!(
                        collector.has_derived_interests(),
                        "{} must derive callback interests with structural_visitor!",
                        metadata.name
                    );
                    diagnostics_only.remove(metadata.name);
                }
                RulePreparation::Semantic(_) => {
                    diagnostics_only.remove(metadata.name);
                }
                RulePreparation::Diagnostics(_) => {}
            }
        }
    }
    assert_eq!(diagnostics_only, expected);
}
