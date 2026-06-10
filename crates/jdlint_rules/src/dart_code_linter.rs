//! Rules ported from dart_code_linter.
//! Implemented in M4 after complexity audit (M0.5.3).

pub mod avoid_dynamic;
pub mod avoid_throw_in_catch_block;
pub mod avoid_nested_conditional_expressions;
pub mod avoid_non_null_assertion;
pub mod avoid_redundant_async;
pub mod avoid_unused_parameters;
pub mod avoid_passing_async_when_sync_expected;
pub mod avoid_unnecessary_type_assertions;
pub mod avoid_unnecessary_type_casts;
pub mod avoid_unrelated_type_assertions;
pub mod avoid_late_keyword;
pub mod avoid_global_state;
pub mod prefer_async_await;
pub mod prefer_correct_identifier_length;
pub mod prefer_conditional_expressions;
pub mod prefer_first;
pub mod prefer_immediate_return;
pub mod prefer_last;
pub mod double_literal_format;
pub mod format_comment;
pub mod member_ordering;
pub mod newline_before_return;
pub mod no_boolean_literal_compare;
pub mod no_empty_block;
pub mod no_equal_arguments;
pub mod no_equal_then_else;
pub mod prefer_moving_to_variable;
pub mod prefer_trailing_comma;
