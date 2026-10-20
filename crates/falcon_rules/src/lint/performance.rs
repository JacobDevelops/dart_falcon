//! Performance lint rules.

pub mod avoid_border_all;
pub mod avoid_shrink_wrap_in_lists;
pub mod avoid_slow_async_io;
pub mod avoid_slow_collection_methods;
pub mod avoid_unnecessary_collections;
pub mod avoid_unnecessary_consumer_widgets;
pub mod avoid_unnecessary_futures;
pub mod avoid_unnecessary_gesture_detector;
pub mod avoid_unnecessary_hook_widgets;
pub mod move_variable_closer_to_its_usage;
pub mod move_variable_outside_iteration;
pub mod noop_primitive_operations;
pub mod prefer_const_border_radius;
pub mod prefer_contains;
pub mod prefer_correct_edge_insets_constructor;
pub mod prefer_declaring_const_constructor;
pub mod prefer_foreach;
pub mod prefer_return_await;
pub mod prefer_single_setstate;
pub mod prefer_use_callback;
pub mod prefer_use_memo;
pub mod prefer_using_list_view;
pub mod sized_box_for_whitespace;
pub mod unnecessary_to_list_in_spreads;
pub mod use_is_even_rather_than_modulo;
pub mod use_string_buffers;
