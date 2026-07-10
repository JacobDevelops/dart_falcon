//! Rules ported from pyramid_lint.
//! Implemented in M4 after complexity audit (M0.5.3).

pub mod avoid_abbreviations_in_doc_comments;
pub mod avoid_empty_blocks;
pub mod avoid_inverted_boolean_expressions;
pub mod avoid_mutable_global_variables;
pub mod avoid_nested_if;
pub mod avoid_positional_fields_in_records;
pub mod avoid_redundant_pattern_field_names;
pub mod avoid_single_child_column_or_row;
pub mod boolean_prefixes;
pub mod class_members_ordering;
pub mod correct_order_for_super_dispose;
pub mod max_lines_for_file;
pub mod max_lines_for_function;
pub mod max_parameters_for_function;
pub mod max_switch_cases;
// M-config: new complexity-metric rules
pub mod cyclomatic_complexity;
pub mod maximum_nesting_level;
pub mod no_duplicate_case_values;
pub mod no_self_comparisons;
pub mod prefer_async_callback;
pub mod prefer_declaring_const_constructor;
pub mod prefer_dedicated_media_query_methods;
pub mod prefer_iterable_any;
pub mod prefer_iterable_every;
pub mod prefer_underscore_for_unused_callback_parameters;
pub mod proper_controller_dispose;
pub mod proper_expanded_and_flexible;
pub mod proper_from_environment;
pub mod proper_super_init_state;
pub mod unnecessary_flutter_imports;
pub mod unnecessary_nullable_return_type;
pub mod use_once_constructors_once_provider;
pub mod use_spacer_as_expanded_child;
// Aliases of shared dart_code_linter implementations, emitted under pyramid_lint rule ids.
pub mod avoid_unused_parameters;
pub mod no_empty_block;
pub mod no_magic_number;
