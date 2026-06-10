//! Rules ported from pyramid_lint.
//! Implemented in M4 after complexity audit (M0.5.3).

pub mod avoid_empty_blocks;
pub mod avoid_inverted_boolean_expressions;
pub mod avoid_nested_if;
pub mod avoid_positional_fields_in_records;
pub mod boolean_prefixes;
pub mod correct_order_for_super_dispose;
pub mod max_lines_for_function;
pub mod max_lines_for_file;
pub mod max_parameters_for_function;
pub mod max_switch_cases;
