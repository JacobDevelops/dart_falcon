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
