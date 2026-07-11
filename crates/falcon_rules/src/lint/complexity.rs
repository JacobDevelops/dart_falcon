//! Complexity lint rules.

pub mod avoid_inverted_boolean_expressions;
pub mod avoid_nested_conditional_expressions;
pub mod avoid_nested_if;
pub mod avoid_redundant_async;
pub mod avoid_unnecessary_type_assertions;
pub mod avoid_unnecessary_type_casts;
pub mod cyclomatic_complexity;
pub mod max_lines_for_file;
pub mod max_lines_for_function;
pub mod max_parameters_for_function;
pub mod max_switch_cases;
pub mod maximum_nesting_level;
pub mod no_boolean_literal_compare;
pub mod prefer_conditional_expressions;
pub mod prefer_extracting_callbacks;
pub mod prefer_immediate_return;
pub mod prefer_iterable_any;
pub mod prefer_iterable_every;
pub mod prefer_moving_to_variable;
