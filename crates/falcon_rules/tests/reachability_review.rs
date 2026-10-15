use falcon_rules::all_rules;

#[path = "support/inference.rs"]
mod inference;

#[test]
fn inherited_inventory_does_not_hide_movable_assertions() {
    inference::assert_lints(
        "prefer-asserts-in-initializer-lists",
        include_str!("corpus/prefer-asserts-in-initializer-lists/inherited_assert_inventory.dart"),
        3,
    );
}

#[test]
fn extension_assertions_retain_the_preceding_class_member_inventory() {
    inference::assert_lints(
        "prefer-asserts-in-initializer-lists",
        include_str!("corpus/prefer-asserts-in-initializer-lists/extension_class_order.dart"),
        2,
    );
}

#[test]
fn inherited_inventory_and_preindexed_extension_reachability() {
    inference::assert_lints(
        "unreachable-from-main",
        include_str!("corpus/unreachable-from-main/inherited_inventory.dart"),
        3,
    );
}

#[test]
fn lexical_bindings_include_loop_pattern_and_closure_locals() {
    inference::assert_lints(
        "avoid-double-and-int-checks",
        include_str!("corpus/avoid-double-and-int-checks/lexical_bindings.dart"),
        3,
    );
}

#[test]
fn constant_type_patterns_use_resolved_static_expression_types() {
    inference::assert_lints(
        "type-literal-in-constant-pattern",
        include_str!("corpus/type-literal-in-constant-pattern/constant_type_values.dart"),
        3,
    );
}

#[test]
fn unawaited_future_diagnostics_retain_grouping_spans() {
    let source = "import 'dart:async';\nFuture<void> load() async {}\nvoid run() async { load(); (load()); ((load())); }\n";
    let diagnostics = inference::analyze(
        &falcon_rules::lint::style::unawaited_futures::UnawaitedFutures,
        std::path::Path::new("grouped_futures.dart"),
        source,
        &falcon_config::FalconConfig::default(),
    );
    let reported: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
        .collect();
    assert_eq!(reported, ["load", "(load())", "((load()))"]);
}

fn assert_without_external_sources(rule_name: &str, source: &str) {
    use falcon_analyze::{
        AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, SemanticModel,
        SignatureIndex, TypeIndex,
    };
    use falcon_config::FalconConfig;
    use std::path::Path;

    let path = Path::new("/unloaded_project/main.dart");
    let (program, errors) = falcon_dart_parser::parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        None,
    );
    let types = TypeIndex::from_resolved_files(
        [(
            path,
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    let model = SemanticModel::new(path, &identities, Some(&types));
    let signatures = SignatureIndex::from_program(&program, &model);
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(path, source, &config)
        .with_types(&types)
        .with_identities(&identities)
        .with_signatures(&signatures);
    let rules = all_rules();
    let rule = &rules.iter().find(|(name, _)| *name == rule_name).unwrap().1;
    inference::assert_diagnostics(source, rule_name, &rule.analyze(&program, &context), 0);
}

#[test]
fn unloaded_sdk_hierarchies_do_not_disprove_overrides() {
    assert_without_external_sources(
        "unreachable-from-main",
        include_str!("corpus/unreachable-from-main/unloaded_sdk_hierarchy.dart"),
    );
}

#[test]
fn unresolved_superclasses_do_not_disprove_assertion_instance_access() {
    // No external library is indexed: the dependency's member namespace is unknown.
    assert_without_external_sources(
        "prefer-asserts-in-initializer-lists",
        "import 'dependency.dart';\nclass Child extends Base {\nChild() { assert(inheritedField != null); }\n}\n",
    );
}
