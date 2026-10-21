//! Suspicious lint rules.

mod bloc_passing;

use crate::meta::{Domain, RuleSource};

crate::declare_rules! {
    Suspicious;
    avoid_accessing_other_classes_private_members::AvoidAccessingOtherClassesPrivateMembers => ("avoid-accessing-other-classes-private-members", resolved, None, false, RuleSource::Falcon);
    avoid_banned_names::AvoidBannedNames => ("avoid-banned-names", file, None, false, RuleSource::Falcon, options = avoid_banned_names::Options);
    avoid_banned_types::AvoidBannedTypes => ("avoid-banned-types", resolved, None, false, RuleSource::Falcon, options = avoid_banned_types::Options);
    avoid_banned_annotations::AvoidBannedAnnotations => ("avoid-banned-annotations", file, None, false, RuleSource::Falcon, options = avoid_banned_annotations::Options);
    avoid_banned_exports::AvoidBannedExports => ("avoid-banned-exports", file, None, false, RuleSource::Falcon, options = avoid_banned_exports::Options);
    banned_usage::BannedUsage => ("banned-usage", resolved, None, false, RuleSource::Falcon, options = banned_usage::Options);
    avoid_barrel_files::AvoidBarrelFiles => ("avoid-barrel-files", file, None, false, RuleSource::Falcon);
    avoid_double_slash_imports::AvoidDoubleSlashImports => ("avoid-double-slash-imports", file, None, false, RuleSource::Falcon);
    avoid_incorrect_uri::AvoidIncorrectUri => ("avoid-incorrect-uri", resolved, None, false, RuleSource::Falcon);
    avoid_referencing_subclasses::AvoidReferencingSubclasses => ("avoid-referencing-subclasses", resolved, None, false, RuleSource::Falcon, options = avoid_referencing_subclasses::Options);
    avoid_suspicious_super_overrides::AvoidSuspiciousSuperOverrides => ("avoid-suspicious-super-overrides", resolved, None, false, RuleSource::Falcon);
    no_magic_string::NoMagicString => ("no-magic-string", file, None, false, RuleSource::Falcon, options = no_magic_string::Options);
    avoid_non_final_exception_class_fields::AvoidNonFinalExceptionClassFields => ("avoid-non-final-exception-class-fields", resolved, None, false, RuleSource::Falcon);
    avoid_mutating_parameters::AvoidMutatingParameters => ("avoid-mutating-parameters", file, None, false, RuleSource::Falcon);
    avoid_nested_assignments::AvoidNestedAssignments => ("avoid-nested-assignments", file, None, false, RuleSource::Falcon, options = avoid_nested_assignments::Options);
    avoid_multi_assignment::AvoidMultiAssignment => ("avoid-multi-assignment", file, None, false, RuleSource::Falcon);
    avoid_negated_conditions::AvoidNegatedConditions => ("avoid-negated-conditions", file, None, false, RuleSource::Falcon, options = avoid_negated_conditions::Options);
    avoid_inverted_boolean_checks::AvoidInvertedBooleanChecks => ("avoid-inverted-boolean-checks", file, None, false, RuleSource::Falcon);
    prefer_unique_test_names::PreferUniqueTestNames => ("prefer-unique-test-names", resolved, Some(Domain::Test), false, RuleSource::Falcon, options = prefer_unique_test_names::Options);
    avoid_top_level_members_in_tests::AvoidTopLevelMembersInTests => ("avoid-top-level-members-in-tests", file, Some(Domain::Test), false, RuleSource::Falcon);
    avoid_commented_out_code::AvoidCommentedOutCode => ("avoid-commented-out-code", file, None, false, RuleSource::Falcon);
    avoid_shadowing::AvoidShadowing => ("avoid-shadowing", resolved, None, false, RuleSource::Falcon);
    avoid_shadowed_extension_methods::AvoidShadowedExtensionMethods => ("avoid-shadowed-extension-methods", resolved, None, false, RuleSource::Falcon);
    avoid_assignments_as_conditions::AvoidAssignmentsAsConditions => ("avoid-assignments-as-conditions", file, None, false, RuleSource::Falcon);
    avoid_bitwise_operators_with_booleans::AvoidBitwiseOperatorsWithBooleans => ("avoid-bitwise-operators-with-booleans", resolved, None, false, RuleSource::Falcon);
    cascade_after_null_coalescing::CascadeAfterNullCoalescing => ("cascade-after-null-coalescing", file, None, false, RuleSource::Falcon);
    no_self_comparisons::NoSelfComparisons => ("no-self-comparisons", resolved, None, true, RuleSource::PyramidLint("no_self_comparisons"));
    avoid_print::AvoidPrint => ("avoid-print", resolved, Some(Domain::Flutter), true, RuleSource::Lints("avoid_print"));
    avoid_empty_else::AvoidEmptyElse => ("avoid-empty-else", file, None, true, RuleSource::Lints("avoid_empty_else"));
    avoid_shadowing_type_parameters::AvoidShadowingTypeParameters => ("avoid-shadowing-type-parameters", file, None, true, RuleSource::Lints("avoid_shadowing_type_parameters"));
    empty_catches::EmptyCatches => ("empty-catches", file, None, true, RuleSource::Lints("empty_catches"));
    no_wildcard_variable_uses::NoWildcardVariableUses => ("no-wildcard-variable-uses", file, None, true, RuleSource::Lints("no_wildcard_variable_uses"));
    avoid_returning_null_for_void::AvoidReturningNullForVoid => ("avoid-returning-null-for-void", file, None, true, RuleSource::Lints("avoid_returning_null_for_void"));
    control_flow_in_finally::ControlFlowInFinally => ("control-flow-in-finally", file, None, true, RuleSource::Lints("control_flow_in_finally"));
    empty_statements::EmptyStatements => ("empty-statements", file, None, true, RuleSource::Lints("empty_statements"));
    recursive_getters::RecursiveGetters => ("recursive-getters", file, None, true, RuleSource::Lints("recursive_getters"));
    unnecessary_null_aware_assignments::UnnecessaryNullAwareAssignments => ("unnecessary-null-aware-assignments", file, None, true, RuleSource::Lints("unnecessary_null_aware_assignments"));
    unnecessary_null_in_if_null_operators::UnnecessaryNullInIfNullOperators => ("unnecessary-null-in-if-null-operators", file, None, true, RuleSource::Lints("unnecessary_null_in_if_null_operators"));
    use_rethrow_when_possible::UseRethrowWhenPossible => ("use-rethrow-when-possible", file, None, true, RuleSource::Lints("use_rethrow_when_possible"));
    avoid_dynamic::AvoidDynamic => ("avoid-dynamic", file, None, false, RuleSource::DartCodeLinter("avoid-dynamic"));
    avoid_ignoring_return_values::AvoidIgnoringReturnValues => ("avoid-ignoring-return-values", resolved, None, false, RuleSource::DartCodeLinter("avoid-ignoring-return-values"));
    avoid_passing_async_when_sync_expected::AvoidPassingAsyncWhenSyncExpected => ("avoid-passing-async-when-sync-expected", resolved, None, true, RuleSource::DartCodeLinter("avoid-passing-async-when-sync-expected"));
    avoid_throw_in_catch_block::AvoidThrowInCatchBlock => ("avoid-throw-in-catch-block", file, None, false, RuleSource::DartCodeLinter("avoid-throw-in-catch-block"));
    avoid_unrelated_type_assertions::AvoidUnrelatedTypeAssertions => ("avoid-unrelated-type-assertions", resolved, None, true, RuleSource::DartCodeLinter("avoid-unrelated-type-assertions"));
    no_empty_block::NoEmptyBlock => ("no-empty-block", file, None, false, RuleSource::DartCodeLinter("no-empty-block"));
    no_equal_arguments::NoEqualArguments => ("no-equal-arguments", file, None, false, RuleSource::DartCodeLinter("no-equal-arguments"));
    no_equal_then_else::NoEqualThenElse => ("no-equal-then-else", file, None, true, RuleSource::DartCodeLinter("no-equal-then-else"));
    no_duplicate_case_values::NoDuplicateCaseValues => ("no-duplicate-case-values", resolved, None, true, RuleSource::PyramidLint("no_duplicate_case_values"));
    overridden_fields::OverriddenFields => ("overridden-fields", resolved, None, true, RuleSource::Lints("overridden_fields"));
    avoid_types_as_parameter_names::AvoidTypesAsParameterNames => ("avoid-types-as-parameter-names", resolved, None, true, RuleSource::Lints("avoid_types_as_parameter_names"));
    avoid_passing_bloc_to_bloc::AvoidPassingBlocToBloc => ("avoid-passing-bloc-to-bloc", resolved, Some(Domain::Bloc), false, RuleSource::Falcon);
    avoid_passing_build_context_to_blocs::AvoidPassingBuildContextToBlocs => ("avoid-passing-build-context-to-blocs", resolved, Some(Domain::Bloc), false, RuleSource::Falcon);
    avoid_passing_bloc_to_widget::AvoidPassingBlocToWidget => ("avoid-passing-bloc-to-widget", resolved, Some(Domain::Bloc), false, RuleSource::Falcon);
}
