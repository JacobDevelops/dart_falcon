//! Ported lint rules from dart_code_linter and pyramid_lint.
//!
//! Rules are registered via `RuleRegistry` in `jdlint_analyze`.
//! Each rule is a zero-sized struct implementing the `Rule` trait.
//!
//! Phase 1 target: ~60 rules from jfit's analysis_options.yaml.
//! Complexity tiers (assigned in M0.5): SIMPLE / MEDIUM / COMPLEX.

pub mod dart_code_linter;
pub mod pyramid_lint;

use jdlint_analyze::Rule;

/// Return all implemented lint rules.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
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
    ]
}
