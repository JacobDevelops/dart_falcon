//! Falcon's lint rules.
//!
//! Rules are organized biome-style by group under `lint/<group>/<rule>.rs`
//! (complexity, correctness, performance, style, suspicious). Each rule is a
//! zero-sized struct implementing the `Rule` trait and registered via
//! `RuleRegistry` in `falcon_analyze`. Upstream provenance is recorded per rule
//! by the `source` field of its `RuleMeta` entry (see `meta.rs`). Project
//! (cross-file) rules live under `project/`.

pub mod lint;
pub mod member_order;
pub mod meta;
pub mod project;
pub mod schema;

use falcon_analyze::{ProjectRule, Rule};
use falcon_config::{FalconConfig, ResolvedSeverity, Rules};
use falcon_diagnostics::{Diagnostic, Severity};

use crate::meta::meta_for;

/// The enabled rule set: every rule that could fire for at least one path.
pub struct ResolvedRules {
    pub rules: Vec<Box<dyn Rule>>,
}

/// The enabled project (cross-file) rule set. CLI-only; the LSP never runs it.
pub struct ResolvedProjectRules {
    pub rules: Vec<Box<dyn ProjectRule>>,
}

/// Resolve the rule set to register from `config`.
///
/// A rule is registered if it is enabled for **any** path — the base config or
/// any override turns it on (see
/// [`falcon_config::FalconConfig::is_rule_enabled_anywhere`]). Per-file severity
/// and off-scoping are applied afterwards by [`apply_severities`], which drops
/// diagnostics that a matching override disables. Shared by the CLI pipeline and
/// the LSP server so both behave identically. Warns about config entries that
/// name no registered rule or sit under the wrong group.
pub fn resolve_rules(config: &FalconConfig) -> ResolvedRules {
    warn_unknown_config(config);

    let mut rules = Vec::new();
    for rule in all_rules() {
        let name = rule.name();
        // Every registered rule has a metadata entry (enforced by tests).
        let Some(meta) = meta_for(name) else {
            eprintln!("warning: rule `{name}` has no metadata entry; skipping");
            continue;
        };
        if config.is_rule_enabled_anywhere(meta.group, meta.name, meta.recommended, meta.domains) {
            rules.push(rule);
        }
    }
    ResolvedRules { rules }
}

/// Apply per-file severity resolution to each diagnostic and drop those the
/// resolved config disables for that file.
///
/// For every diagnostic, the base rule resolution is patched by any override
/// whose `includes` match the diagnostic's `file_path` (see
/// [`falcon_config::FalconConfig::resolve_rule_for`]): a resolved severity
/// rewrites `diag.severity`; `None` (rule off for this path) removes the
/// diagnostic. Diagnostics whose rule has no metadata entry are left unchanged.
///
/// Correctness-first: globs are matched per diagnostic. If this ever shows up in
/// profiles, cache the per-path resolution (keyed by `file_path`).
pub fn apply_severities(diags: &mut Vec<Diagnostic>, config: &FalconConfig) {
    diags.retain_mut(|diag| {
        let Some(meta) = meta_for(diag.rule) else {
            return true;
        };
        // Route by rule kind: project rules resolve against the `project` config
        // path, file rules against `linter`.
        let resolved = if meta.project {
            config.resolve_project_rule_for(
                &diag.file_path,
                meta.group,
                meta.name,
                meta.recommended,
            )
        } else {
            config.resolve_rule_for(
                &diag.file_path,
                meta.group,
                meta.name,
                meta.recommended,
                meta.domains,
            )
        };
        match resolved {
            Some(sev) => {
                diag.severity = to_severity(sev);
                true
            }
            None => false,
        }
    });
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
/// are placed under a group the rule does not belong to — across the base
/// `linter.rules` and every `overrides[].linter.rules` entry.
fn warn_unknown_config(config: &FalconConfig) {
    for warning in config_warnings(config) {
        eprintln!("{warning}");
    }
}

/// Collect the config warnings that [`warn_unknown_config`] prints. Returning
/// them (rather than printing inline) keeps the check unit-testable. Each entry
/// names its source context — `falcon.json` for the base config, `overrides[i]`
/// for the i-th override — so a warning points the user at the offending block.
fn config_warnings(config: &FalconConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    check_rule_groups(
        &config.linter.rules,
        "falcon.json",
        Section::Linter,
        &mut warnings,
    );
    check_rule_groups(
        &config.project.rules,
        "falcon.json",
        Section::Project,
        &mut warnings,
    );
    for (idx, over) in config.overrides.iter().enumerate() {
        let ctx = format!("overrides[{idx}]");
        if let Some(linter) = &over.linter {
            check_rule_groups(&linter.rules, &ctx, Section::Linter, &mut warnings);
        }
        if let Some(project) = &over.project {
            check_rule_groups(&project.rules, &ctx, Section::Project, &mut warnings);
        }
    }
    warnings
}

/// Which config section a `Rules` block came from, so misplaced rules can be
/// steered to the right one.
#[derive(Clone, Copy)]
enum Section {
    Linter,
    Project,
}

/// Push a warning for every rule in `rules` that names no registered rule, sits
/// under the wrong group, or is configured in the wrong top-level section (a
/// project rule under `linter.rules`, or a file rule under `project.rules`).
/// `ctx` labels the source block (e.g. `falcon.json` or `overrides[1]`).
fn check_rule_groups(rules: &Rules, ctx: &str, section: Section, warnings: &mut Vec<String>) {
    for (group, group_rules) in &rules.groups {
        for name in group_rules.keys() {
            let Some(meta) = meta_for(name) else {
                warnings.push(format!(
                    "warning: {ctx} configures unknown rule `{name}` (under group `{group}`)"
                ));
                continue;
            };
            if meta.group != group.as_str() {
                warnings.push(format!(
                    "warning: {ctx} configures rule `{name}` under group `{group}`, \
                     but it belongs to `{}`",
                    meta.group
                ));
                continue;
            }
            match (section, meta.project) {
                (Section::Linter, true) => warnings.push(format!(
                    "warning: {ctx} configures `{name}` under `linter.rules`, but it is a project \
                     rule; configure it under `project.rules`"
                )),
                (Section::Project, false) => warnings.push(format!(
                    "warning: {ctx} configures `{name}` under `project.rules`, but it is a \
                     file-level rule; configure it under `linter.rules`"
                )),
                _ => {}
            }
        }
    }
}

/// Return all implemented lint rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // ── complexity ──
        Box::new(lint::complexity::avoid_inverted_boolean_expressions::AvoidInvertedBooleanExpressions),
        Box::new(lint::complexity::avoid_nested_conditional_expressions::AvoidNestedConditionalExpressions),
        Box::new(lint::complexity::avoid_nested_if::AvoidNestedIf),
        Box::new(lint::complexity::avoid_redundant_async::AvoidRedundantAsync),
        Box::new(lint::complexity::avoid_unnecessary_type_assertions::AvoidUnnecessaryTypeAssertions),
        Box::new(lint::complexity::avoid_unnecessary_type_casts::AvoidUnnecessaryTypeCasts),
        Box::new(lint::complexity::cyclomatic_complexity::CyclomaticComplexity),
        Box::new(lint::complexity::max_lines_for_file::MaxLinesForFile),
        Box::new(lint::complexity::max_lines_for_function::MaxLinesForFunction),
        Box::new(lint::complexity::max_parameters_for_function::MaxParametersForFunction),
        Box::new(lint::complexity::max_switch_cases::MaxSwitchCases),
        Box::new(lint::complexity::maximum_nesting_level::MaximumNestingLevel),
        Box::new(lint::complexity::no_boolean_literal_compare::NoBooleanLiteralCompare),
        Box::new(lint::complexity::prefer_conditional_expressions::PreferConditionalExpressions),
        Box::new(lint::complexity::prefer_extracting_callbacks::PreferExtractingCallbacks),
        Box::new(lint::complexity::prefer_immediate_return::PreferImmediateReturn),
        Box::new(lint::complexity::prefer_iterable_any::PreferIterableAny),
        Box::new(lint::complexity::prefer_iterable_every::PreferIterableEvery),
        Box::new(lint::complexity::prefer_moving_to_variable::PreferMovingToVariable),
        // ── correctness ──
        Box::new(lint::correctness::avoid_global_state::AvoidGlobalState),
        Box::new(lint::correctness::avoid_mutable_global_variables::AvoidMutableGlobalVariables),
        Box::new(lint::correctness::avoid_returning_widgets::AvoidReturningWidgets),
        Box::new(lint::correctness::avoid_unused_parameters::AvoidUnusedParameters),
        Box::new(lint::correctness::avoid_unused_parameters::AvoidUnusedParametersPyramid),
        Box::new(lint::correctness::correct_order_for_super_dispose::CorrectOrderForSuperDispose),
        Box::new(lint::correctness::proper_controller_dispose::ProperControllerDispose),
        Box::new(lint::correctness::proper_expanded_and_flexible::ProperExpandedAndFlexible),
        Box::new(lint::correctness::proper_from_environment::ProperFromEnvironment),
        Box::new(lint::correctness::proper_super_init_state::ProperSuperInitState),
        Box::new(lint::correctness::unnecessary_flutter_imports::UnnecessaryFlutterImports),
        Box::new(lint::correctness::unnecessary_nullable_return_type::UnnecessaryNullableReturnType),
        Box::new(lint::correctness::use_once_constructors_once_provider::UseOnceConstructorsOnceProvider),
        // ── performance ──
        Box::new(lint::performance::prefer_const_border_radius::PreferConstBorderRadius),
        Box::new(lint::performance::prefer_correct_edge_insets_constructor::PreferCorrectEdgeInsetsConstructor),
        Box::new(lint::performance::prefer_declaring_const_constructor::PreferDeclaringConstConstructor),
        // ── style ──
        Box::new(lint::style::avoid_abbreviations_in_doc_comments::AvoidAbbreviationsInDocComments),
        Box::new(lint::style::avoid_late_keyword::AvoidLateKeyword),
        Box::new(lint::style::avoid_non_null_assertion::AvoidNonNullAssertion),
        Box::new(lint::style::avoid_positional_fields_in_records::AvoidPositionalFieldsInRecords),
        Box::new(lint::style::avoid_redundant_pattern_field_names::AvoidRedundantPatternFieldNames),
        Box::new(lint::style::avoid_single_child_column_or_row::AvoidSingleChildColumnOrRow),
        Box::new(lint::style::avoid_top_level_member_access::AvoidTopLevelMemberAccess),
        Box::new(lint::style::binary_expression_operand_order::BinaryExpressionOperandOrder),
        Box::new(lint::style::boolean_prefixes::BooleanPrefixes),
        Box::new(lint::style::class_members_ordering::ClassMembersOrdering),
        Box::new(lint::style::double_literal_format::DoubleLiteralFormat),
        Box::new(lint::style::format_comment::FormatComment),
        Box::new(lint::style::member_ordering::MemberOrdering),
        Box::new(lint::style::newline_before_return::NewlineBeforeReturn),
        Box::new(lint::style::no_magic_number::NoMagicNumber),
        Box::new(lint::style::no_magic_number::NoMagicNumberPyramid),
        Box::new(lint::style::no_object_declaration::NoObjectDeclaration),
        Box::new(lint::style::prefer_async_await::PreferAsyncAwait),
        Box::new(lint::style::prefer_async_callback::PreferAsyncCallback),
        Box::new(lint::style::prefer_correct_identifier_length::PreferCorrectIdentifierLength),
        Box::new(lint::style::prefer_correct_type_name::PreferCorrectTypeName),
        Box::new(lint::style::prefer_dedicated_media_query_methods::PreferDedicatedMediaQueryMethods),
        Box::new(lint::style::prefer_first::PreferFirst),
        Box::new(lint::style::prefer_iterable_of::PreferIterableOf),
        Box::new(lint::style::prefer_last::PreferLast),
        Box::new(lint::style::prefer_trailing_comma::PreferTrailingComma),
        Box::new(lint::style::prefer_underscore_for_unused_callback_parameters::PreferUnderscoreForUnusedCallbackParameters),
        Box::new(lint::style::use_design_system_item::UseDesignSystemItem),
        Box::new(lint::style::use_spacer_as_expanded_child::UseSpacerAsExpandedChild),
        // ── suspicious ──
        Box::new(lint::suspicious::avoid_dynamic::AvoidDynamic),
        Box::new(lint::suspicious::avoid_empty_blocks::AvoidEmptyBlocks),
        Box::new(lint::suspicious::avoid_ignoring_return_values::AvoidIgnoringReturnValues),
        Box::new(lint::suspicious::avoid_passing_async_when_sync_expected::AvoidPassingAsyncWhenSyncExpected),
        Box::new(lint::suspicious::avoid_throw_in_catch_block::AvoidThrowInCatchBlock),
        Box::new(lint::suspicious::avoid_unrelated_type_assertions::AvoidUnrelatedTypeAssertions),
        Box::new(lint::suspicious::no_duplicate_case_values::NoDuplicateCaseValues),
        Box::new(lint::suspicious::no_empty_block::NoEmptyBlock),
        Box::new(lint::suspicious::no_empty_block::NoEmptyBlockPyramid),
        Box::new(lint::suspicious::no_equal_arguments::NoEqualArguments),
        Box::new(lint::suspicious::no_equal_then_else::NoEqualThenElse),
        Box::new(lint::suspicious::no_self_comparisons::NoSelfComparisons),
    ]
}

/// Return all implemented project (cross-file) rules. These share the metadata
/// table and config schema with per-file rules, but run in the CLI's project
/// pass over every parsed file at once (see `falcon_analyze::ProjectRule`).
pub fn all_project_rules() -> Vec<Box<dyn ProjectRule>> {
    vec![
        Box::new(project::unused_files::UnusedFiles),
        Box::new(project::unused_code::UnusedCode),
        Box::new(project::unnecessary_nullable::UnnecessaryNullable),
    ]
}

/// Resolve the enabled project rule set from `config`, using the same
/// enablement semantics as [`resolve_rules`] (a rule registers if it is enabled
/// for any path). Rules with no metadata entry are skipped with a warning.
pub fn resolve_project_rules(config: &FalconConfig) -> ResolvedProjectRules {
    let mut rules = Vec::new();
    for rule in all_project_rules() {
        let name = rule.name();
        let Some(meta) = meta_for(name) else {
            eprintln!("warning: project rule `{name}` has no metadata entry; skipping");
            continue;
        };
        if config.is_project_rule_enabled_anywhere(meta.group, meta.name, meta.recommended) {
            rules.push(rule);
        }
    }
    ResolvedProjectRules { rules }
}

#[cfg(test)]
mod tests {
    use super::config_warnings;
    use falcon_config::FalconConfig;

    /// `config_warnings` flags unknown/misgrouped rules in the base config *and*
    /// in every override, tagging each with its source block.
    #[test]
    fn config_warnings_cover_base_and_overrides() {
        let config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": {
                    "style": { "totally_fake_rule": "warn" }
                }
            },
            "overrides": [
                {
                    "includes": ["a/**"],
                    "linter": { "rules": { "correctness": { "another_fake_rule": "warn" } } }
                },
                {
                    "includes": ["b/**"],
                    // `no_magic_number` is a real rule, but it belongs to `style`.
                    "linter": { "rules": { "complexity": { "no_magic_number": "warn" } } }
                }
            ]
        }))
        .expect("valid config");

        let warnings = config_warnings(&config);

        assert!(
            warnings.iter().any(
                |w| w.contains("falcon.json") && w.contains("unknown rule `totally_fake_rule`")
            ),
            "base unknown-rule warning missing: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("overrides[0]")
                    && w.contains("unknown rule `another_fake_rule`")),
            "overrides[0] unknown-rule warning missing: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("overrides[1]")
                && w.contains("`no_magic_number`")
                && w.contains("belongs to `style`")),
            "overrides[1] wrong-group warning missing: {warnings:?}"
        );
        assert_eq!(warnings.len(), 3, "unexpected warnings: {warnings:?}");
    }

    /// A project rule under `linter.rules` (and a file rule under `project.rules`)
    /// is flagged with a message steering it to the correct section.
    #[test]
    fn config_warnings_flag_wrong_section() {
        let config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": { "correctness": { "unused-files": "warn" } }
            },
            "project": {
                "rules": { "suspicious": { "avoid-dynamic": "warn" } }
            }
        }))
        .expect("valid config");

        let warnings = config_warnings(&config);

        assert!(
            warnings.iter().any(|w| w.contains("`unused-files`")
                && w.contains("project rule")
                && w.contains("project.rules")),
            "missing project-rule-under-linter warning: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("`avoid-dynamic`")
                && w.contains("file-level rule")
                && w.contains("linter.rules")),
            "missing file-rule-under-project warning: {warnings:?}"
        );
        assert_eq!(warnings.len(), 2, "unexpected warnings: {warnings:?}");
    }
}
