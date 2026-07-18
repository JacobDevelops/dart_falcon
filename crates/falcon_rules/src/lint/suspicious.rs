//! Suspicious lint rules.

pub mod avoid_dynamic;
pub mod avoid_empty_else;
pub mod avoid_ignoring_return_values;
pub mod avoid_passing_async_when_sync_expected;
pub mod avoid_print;
pub mod avoid_returning_null_for_void;
pub mod avoid_shadowing_type_parameters;
pub mod avoid_throw_in_catch_block;
pub mod avoid_unrelated_type_assertions;
pub mod control_flow_in_finally;
pub mod empty_catches;
pub mod empty_statements;
pub mod no_duplicate_case_values;
pub mod no_empty_block;
pub mod no_equal_arguments;
pub mod no_equal_then_else;
pub mod no_self_comparisons;
pub mod no_wildcard_variable_uses;
pub mod recursive_getters;
pub mod unnecessary_null_aware_assignments;
pub mod unnecessary_null_in_if_null_operators;
pub mod use_rethrow_when_possible;
