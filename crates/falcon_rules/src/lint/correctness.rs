//! Correctness lint rules.

pub mod avoid_global_state;
pub mod avoid_mutable_global_variables;
pub mod avoid_relative_lib_imports;
pub mod avoid_returning_widgets;
pub mod avoid_unused_parameters;
pub mod avoid_web_libraries_in_flutter;
pub mod correct_order_for_super_dispose;
pub mod hash_and_equals;
pub mod implementation_imports;
pub mod no_logic_in_create_state;
pub mod proper_controller_dispose;
pub mod proper_expanded_and_flexible;
pub mod proper_from_environment;
pub mod proper_super_init_state;
pub mod unnecessary_flutter_imports;
pub mod unnecessary_nullable_return_type;
pub mod use_once_constructors_once_provider;
pub mod valid_regexps;
