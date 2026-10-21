//! Complexity lint rules.

use crate::meta::{Domain, RuleSource};

crate::declare_rules! {
    Complexity;
    avoid_unnecessary_containers::AvoidUnnecessaryContainers => ("avoid-unnecessary-containers", file, Some(Domain::Flutter), true, RuleSource::Lints("avoid_unnecessary_containers"));
    unnecessary_overrides::UnnecessaryOverrides => ("unnecessary-overrides", resolved, None, true, RuleSource::Lints("unnecessary_overrides"));
    avoid_function_literals_in_foreach_calls::AvoidFunctionLiteralsInForeachCalls => ("avoid-function-literals-in-foreach-calls", resolved, None, false, RuleSource::Lints("avoid_function_literals_in_foreach_calls"));
    prefer_conditional_assignment::PreferConditionalAssignment => ("prefer-conditional-assignment", resolved, None, true, RuleSource::Lints("prefer_conditional_assignment"));
    prefer_for_elements_to_map_from_iterable::PreferForElementsToMapFromIterable => ("prefer-for-elements-to-map-from-iterable", file, None, true, RuleSource::Lints("prefer_for_elements_to_map_fromIterable"));
    prefer_if_null_operators::PreferIfNullOperators => ("prefer-if-null-operators", file, None, true, RuleSource::Lints("prefer_if_null_operators"));
    prefer_null_aware_operators::PreferNullAwareOperators => ("prefer-null-aware-operators", file, None, true, RuleSource::Lints("prefer_null_aware_operators"));
    unnecessary_const::UnnecessaryConst => ("unnecessary-const", file, None, true, RuleSource::Lints("unnecessary_const"));
    unnecessary_getters_setters::UnnecessaryGettersSetters => ("unnecessary-getters-setters", file, None, true, RuleSource::Lints("unnecessary_getters_setters"));
    avoid_nested_conditional_expressions::AvoidNestedConditionalExpressions => ("avoid-nested-conditional-expressions", file, None, false, RuleSource::DartCodeLinter("avoid-nested-conditional-expressions"));
    avoid_redundant_async::AvoidRedundantAsync => ("avoid-redundant-async", file, None, true, RuleSource::DartCodeLinter("avoid-redundant-async"));
    avoid_unnecessary_type_assertions::AvoidUnnecessaryTypeAssertions => ("avoid-unnecessary-type-assertions", resolved, None, true, RuleSource::DartCodeLinter("avoid-unnecessary-type-assertions"));
    avoid_unnecessary_type_casts::AvoidUnnecessaryTypeCasts => ("avoid-unnecessary-type-casts", resolved, None, true, RuleSource::DartCodeLinter("avoid-unnecessary-type-casts"));
    avoid_inverted_boolean_expressions::AvoidInvertedBooleanExpressions => ("avoid-inverted-boolean-expressions", file, None, true, RuleSource::PyramidLint("avoid_inverted_boolean_expressions"));
    avoid_nested_if::AvoidNestedIf => ("avoid-nested-if", file, None, false, RuleSource::PyramidLint("avoid_nested_if"));
    max_lines_for_file::MaxLinesForFile => ("max-lines-for-file", file, None, false, RuleSource::PyramidLint("max_lines_for_file"), options = max_lines_for_file::Options);
    max_lines_for_function::MaxLinesForFunction => ("max-lines-for-function", file, None, false, RuleSource::PyramidLint("max_lines_for_function"), options = max_lines_for_function::Options);
    max_parameters_for_function::MaxParametersForFunction => ("max-parameters-for-function", file, None, false, RuleSource::PyramidLint("max_parameters_for_function"), options = max_parameters_for_function::Options);
    widget_tree_depth::WidgetTreeDepth => ("widget-tree-depth", resolved, Some(Domain::Flutter), false, RuleSource::Falcon, options = widget_tree_depth::Options);
    avoid_long_records::AvoidLongRecords => ("avoid-long-records", file, None, false, RuleSource::Falcon, options = avoid_long_records::Options);
    avoid_complex_arithmetic_expressions::AvoidComplexArithmeticExpressions => ("avoid-complex-arithmetic-expressions", resolved, None, false, RuleSource::Falcon, options = avoid_complex_arithmetic_expressions::Options);
    avoid_if_with_many_branches::AvoidIfWithManyBranches => ("avoid-if-with-many-branches", file, None, false, RuleSource::Falcon, options = avoid_if_with_many_branches::Options);
    avoid_nested_switches::AvoidNestedSwitches => ("avoid-nested-switches", file, None, false, RuleSource::Falcon, options = avoid_nested_switches::Options);
    avoid_nested_switch_expressions::AvoidNestedSwitchExpressions => ("avoid-nested-switch-expressions", file, None, false, RuleSource::Falcon, options = avoid_nested_switch_expressions::Options);
    avoid_nested_try_statements::AvoidNestedTryStatements => ("avoid-nested-try-statements", file, None, false, RuleSource::Falcon, options = avoid_nested_try_statements::Options);
    max_statements::MaxStatements => ("max-statements", file, None, false, RuleSource::Falcon, options = max_statements::Options);
    cognitive_complexity::CognitiveComplexity => ("cognitive-complexity", file, None, false, RuleSource::Falcon, options = cognitive_complexity::Options);
    max_switch_cases::MaxSwitchCases => ("max-switch-cases", file, None, false, RuleSource::PyramidLint("max_switch_cases"), options = max_switch_cases::Options);
    cyclomatic_complexity::CyclomaticComplexity => ("cyclomatic-complexity", file, None, false, RuleSource::PyramidLint("cyclomatic_complexity"), options = cyclomatic_complexity::Options);
    maximum_nesting_level::MaximumNestingLevel => ("maximum-nesting-level", file, None, false, RuleSource::PyramidLint("maximum_nesting_level"), options = maximum_nesting_level::Options);
    no_boolean_literal_compare::NoBooleanLiteralCompare => ("no-boolean-literal-compare", resolved, None, true, RuleSource::DartCodeLinter("no-boolean-literal-compare"));
    prefer_conditional_expressions::PreferConditionalExpressions => ("prefer-conditional-expressions", file, None, false, RuleSource::DartCodeLinter("prefer-conditional-expressions"));
    prefer_extracting_callbacks::PreferExtractingCallbacks => ("prefer-extracting-callbacks", resolved, Some(Domain::Flutter), false, RuleSource::DartCodeLinter("prefer-extracting-callbacks"), options = prefer_extracting_callbacks::Options);
    prefer_immediate_return::PreferImmediateReturn => ("prefer-immediate-return", file, None, false, RuleSource::DartCodeLinter("prefer-immediate-return"));
    prefer_moving_to_variable::PreferMovingToVariable => ("prefer-moving-to-variable", resolved, None, false, RuleSource::DartCodeLinter("prefer-moving-to-variable"), options = prefer_moving_to_variable::Options);
    prefer_iterable_any::PreferIterableAny => ("prefer-iterable-any", file, None, true, RuleSource::PyramidLint("prefer_iterable_any"));
    prefer_iterable_every::PreferIterableEvery => ("prefer-iterable-every", file, None, true, RuleSource::PyramidLint("prefer_iterable_every"));
    avoid_complex_conditions::AvoidComplexConditions => ("avoid-complex-conditions", file, None, false, RuleSource::Falcon, options = avoid_complex_conditions::Options);
    avoid_complex_loop_conditions::AvoidComplexLoopConditions => ("avoid-complex-loop-conditions", file, None, false, RuleSource::Falcon, options = avoid_complex_loop_conditions::Options);
    avoid_collapsible_if::AvoidCollapsibleIf => ("avoid-collapsible-if", file, None, false, RuleSource::Falcon);
    one_member_abstracts::OneMemberAbstracts => ("one-member-abstracts", file, None, false, RuleSource::Lints("one_member_abstracts"));
    prefer_expression_function_bodies::PreferExpressionFunctionBodies => ("prefer-expression-function-bodies", file, None, false, RuleSource::Lints("prefer_expression_function_bodies"));
    join_return_with_assignment::JoinReturnWithAssignment => ("join-return-with-assignment", resolved, None, false, RuleSource::Lints("join_return_with_assignment"));
    avoid_classes_with_only_static_members::AvoidClassesWithOnlyStaticMembers => ("avoid-classes-with-only-static-members", resolved, None, false, RuleSource::Lints("avoid_classes_with_only_static_members"));
    cascade_invocations::CascadeInvocations => ("cascade-invocations", resolved, None, false, RuleSource::Lints("cascade_invocations"));
    prefer_if_elements_to_conditional_expressions::PreferIfElementsToConditionalExpressions => ("prefer-if-elements-to-conditional-expressions", file, None, false, RuleSource::Lints("prefer_if_elements_to_conditional_expressions"));
    unnecessary_lambdas::UnnecessaryLambdas => ("unnecessary-lambdas", resolved, None, true, RuleSource::Lints("unnecessary_lambdas"));
}
