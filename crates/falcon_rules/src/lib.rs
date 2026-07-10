//! Ported lint rules from dart_code_linter and pyramid_lint.
//!
//! Rules are registered via `RuleRegistry` in `falcon_analyze`.
//! Each rule is a zero-sized struct implementing the `Rule` trait.
//!
//! Phase 1 target: ~60 rules from jfit's analysis_options.yaml.
//! Complexity tiers (assigned in M0.5): SIMPLE / MEDIUM / COMPLEX.

pub mod dart_code_linter;
pub mod meta;
pub mod pyramid_lint;

use std::collections::HashMap;

use falcon_analyze::Rule;
use falcon_config::{FalconConfig, ResolvedSeverity};
use falcon_diagnostics::{Diagnostic, Severity};

use crate::meta::meta_for;

/// The enabled rule set plus each enabled rule's resolved severity.
pub struct ResolvedRules {
    pub rules: Vec<Box<dyn Rule>>,
    /// Rule name → resolved severity, for rewriting diagnostics post-analysis.
    pub severities: HashMap<&'static str, Severity>,
}

/// Resolve the enabled rule set and each rule's severity from `config`.
///
/// Enablement follows the biome-style resolution in
/// [`falcon_config::LinterConfig::resolve_rule`], driven by each rule's static
/// [`meta`] entry (group, domains, recommended). Shared by the CLI pipeline
/// and the LSP server so both behave identically. Warns about config entries
/// that name no registered rule or sit under the wrong group.
pub fn resolve_rules(config: &FalconConfig) -> ResolvedRules {
    warn_unknown_config(config);

    let mut rules = Vec::new();
    let mut severities = HashMap::new();
    for rule in all_rules() {
        let name = rule.name();
        // Every registered rule has a metadata entry (enforced by tests).
        let Some(meta) = meta_for(name) else {
            eprintln!("warning: rule `{name}` has no metadata entry; skipping");
            continue;
        };
        if let Some(sev) =
            config.resolve_rule(meta.group, meta.name, meta.recommended, meta.domains)
        {
            severities.insert(name, to_severity(sev));
            rules.push(rule);
        }
    }
    ResolvedRules { rules, severities }
}

/// Rewrite each diagnostic's severity to its resolved value. Diagnostics whose
/// rule is absent from `severities` are left unchanged.
pub fn apply_severities(diags: &mut [Diagnostic], severities: &HashMap<&'static str, Severity>) {
    for diag in diags.iter_mut() {
        if let Some(&severity) = severities.get(diag.rule) {
            diag.severity = severity;
        }
    }
}

/// Thin wrapper returning only the enabled rule set (severities discarded).
pub fn enabled_rules(config: &FalconConfig) -> Vec<Box<dyn Rule>> {
    resolve_rules(config).rules
}

fn to_severity(sev: ResolvedSeverity) -> Severity {
    match sev {
        ResolvedSeverity::Info => Severity::Info,
        ResolvedSeverity::Warn => Severity::Warning,
        ResolvedSeverity::Error => Severity::Error,
    }
}

/// Warn about configured rule entries that match no registered rule, or that
/// are placed under a group the rule does not belong to.
fn warn_unknown_config(config: &FalconConfig) {
    for (group, rules) in &config.linter.rules.groups {
        for name in rules.keys() {
            match meta_for(name) {
                None => eprintln!(
                    "warning: falcon.json configures unknown rule `{name}` (under group `{group}`)"
                ),
                Some(meta) if meta.group != group.as_str() => eprintln!(
                    "warning: falcon.json configures rule `{name}` under group `{group}`, \
                     but it belongs to `{}`",
                    meta.group
                ),
                _ => {}
            }
        }
    }
}

/// Return all implemented lint rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // M4.5 — pyramid_lint
        Box::new(pyramid_lint::avoid_empty_blocks::AvoidEmptyBlocks),
        Box::new(pyramid_lint::avoid_inverted_boolean_expressions::AvoidInvertedBooleanExpressions),
        Box::new(pyramid_lint::avoid_nested_if::AvoidNestedIf),
        Box::new(pyramid_lint::avoid_positional_fields_in_records::AvoidPositionalFieldsInRecords),
        Box::new(pyramid_lint::boolean_prefixes::BooleanPrefixes),
        Box::new(pyramid_lint::correct_order_for_super_dispose::CorrectOrderForSuperDispose),
        Box::new(pyramid_lint::max_lines_for_file::MaxLinesForFile),
        Box::new(pyramid_lint::max_lines_for_function::MaxLinesForFunction),
        Box::new(pyramid_lint::max_parameters_for_function::MaxParametersForFunction),
        Box::new(pyramid_lint::max_switch_cases::MaxSwitchCases),
        // M4.5 — dart_code_linter
        Box::new(dart_code_linter::no_magic_number::NoMagicNumber),
        Box::new(dart_code_linter::no_object_declaration::NoObjectDeclaration),
        Box::new(dart_code_linter::avoid_dynamic::AvoidDynamic),
        Box::new(dart_code_linter::avoid_throw_in_catch_block::AvoidThrowInCatchBlock),
        Box::new(dart_code_linter::avoid_nested_conditional_expressions::AvoidNestedConditionalExpressions),
        Box::new(dart_code_linter::avoid_non_null_assertion::AvoidNonNullAssertion),
        Box::new(dart_code_linter::avoid_redundant_async::AvoidRedundantAsync),
        Box::new(dart_code_linter::avoid_unused_parameters::AvoidUnusedParameters),
        Box::new(dart_code_linter::avoid_passing_async_when_sync_expected::AvoidPassingAsyncWhenSyncExpected),
        Box::new(dart_code_linter::avoid_unnecessary_type_assertions::AvoidUnnecessaryTypeAssertions),
        Box::new(dart_code_linter::avoid_unnecessary_type_casts::AvoidUnnecessaryTypeCasts),
        Box::new(dart_code_linter::avoid_unrelated_type_assertions::AvoidUnrelatedTypeAssertions),
        Box::new(dart_code_linter::avoid_late_keyword::AvoidLateKeyword),
        Box::new(dart_code_linter::avoid_global_state::AvoidGlobalState),
        Box::new(dart_code_linter::prefer_async_await::PreferAsyncAwait),
        Box::new(dart_code_linter::prefer_correct_identifier_length::PreferCorrectIdentifierLength),
        Box::new(dart_code_linter::prefer_conditional_expressions::PreferConditionalExpressions),
        Box::new(dart_code_linter::prefer_first::PreferFirst),
        Box::new(dart_code_linter::prefer_immediate_return::PreferImmediateReturn),
        Box::new(dart_code_linter::prefer_last::PreferLast),
        Box::new(dart_code_linter::double_literal_format::DoubleLiteralFormat),
        Box::new(dart_code_linter::format_comment::FormatComment),
        Box::new(dart_code_linter::member_ordering::MemberOrdering),
        Box::new(dart_code_linter::newline_before_return::NewlineBeforeReturn),
        Box::new(dart_code_linter::no_boolean_literal_compare::NoBooleanLiteralCompare),
        Box::new(dart_code_linter::no_empty_block::NoEmptyBlock),
        Box::new(dart_code_linter::no_equal_arguments::NoEqualArguments),
        Box::new(dart_code_linter::no_equal_then_else::NoEqualThenElse),
        Box::new(dart_code_linter::prefer_moving_to_variable::PreferMovingToVariable),
        Box::new(dart_code_linter::prefer_trailing_comma::PreferTrailingComma),
        // M4.6 — dart_code_linter
        Box::new(dart_code_linter::binary_expression_operand_order::BinaryExpressionOperandOrder),
        Box::new(dart_code_linter::avoid_ignoring_return_values::AvoidIgnoringReturnValues),
        Box::new(dart_code_linter::avoid_top_level_member_access::AvoidTopLevelMemberAccess),
        Box::new(dart_code_linter::prefer_const_border_radius::PreferConstBorderRadius),
        Box::new(dart_code_linter::prefer_correct_edge_insets_constructor::PreferCorrectEdgeInsetsConstructor),
        Box::new(dart_code_linter::avoid_returning_widgets::AvoidReturningWidgets),
        Box::new(dart_code_linter::prefer_extracting_callbacks::PreferExtractingCallbacks),
        // Audit gap-fill — dart_code_linter rules named in plan M4.3/M4.5
        Box::new(dart_code_linter::prefer_iterable_of::PreferIterableOf),
        Box::new(dart_code_linter::prefer_correct_type_name::PreferCorrectTypeName),
        Box::new(dart_code_linter::use_design_system_item::UseDesignSystemItem),
        // M4.6 — pyramid_lint
        Box::new(pyramid_lint::use_spacer_as_expanded_child::UseSpacerAsExpandedChild),
        Box::new(pyramid_lint::prefer_dedicated_media_query_methods::PreferDedicatedMediaQueryMethods),
        Box::new(pyramid_lint::prefer_iterable_any::PreferIterableAny),
        Box::new(pyramid_lint::prefer_iterable_every::PreferIterableEvery),
        Box::new(pyramid_lint::prefer_underscore_for_unused_callback_parameters::PreferUnderscoreForUnusedCallbackParameters),
        Box::new(pyramid_lint::no_duplicate_case_values::NoDuplicateCaseValues),
        Box::new(pyramid_lint::prefer_declaring_const_constructor::PreferDeclaringConstConstructor),
        Box::new(pyramid_lint::avoid_abbreviations_in_doc_comments::AvoidAbbreviationsInDocComments),
        Box::new(pyramid_lint::avoid_mutable_global_variables::AvoidMutableGlobalVariables),
        Box::new(pyramid_lint::unnecessary_flutter_imports::UnnecessaryFlutterImports),
        Box::new(pyramid_lint::class_members_ordering::ClassMembersOrdering),
        Box::new(pyramid_lint::unnecessary_nullable_return_type::UnnecessaryNullableReturnType),
        Box::new(pyramid_lint::use_once_constructors_once_provider::UseOnceConstructorsOnceProvider),
        // M4.6 — pyramid_lint aliases of shared dart_code_linter implementations
        Box::new(pyramid_lint::no_empty_block::NoEmptyBlock),
        Box::new(pyramid_lint::no_magic_number::NoMagicNumber),
        Box::new(pyramid_lint::avoid_unused_parameters::AvoidUnusedParameters),
    ]
}
