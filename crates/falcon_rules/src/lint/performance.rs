//! Performance lint rules.

use crate::meta::{Domain, RuleSource};

crate::declare_rules! {
    Performance;
    prefer_single_setstate::PreferSingleSetstate => ("prefer-single-setstate", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    avoid_border_all::AvoidBorderAll => ("avoid-border-all", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    prefer_using_list_view::PreferUsingListView => ("prefer-using-list-view", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    avoid_unnecessary_gesture_detector::AvoidUnnecessaryGestureDetector => ("avoid-unnecessary-gesture-detector", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    sized_box_for_whitespace::SizedBoxForWhitespace => ("sized-box-for-whitespace", file, Some(Domain::Flutter), true, RuleSource::Lints("sized_box_for_whitespace"));
    unnecessary_to_list_in_spreads::UnnecessaryToListInSpreads => ("unnecessary-to-list-in-spreads", file, None, true, RuleSource::Lints("unnecessary_to_list_in_spreads"));
    prefer_const_border_radius::PreferConstBorderRadius => ("prefer-const-border-radius", file, Some(Domain::Flutter), true, RuleSource::DartCodeLinter("prefer-const-border-radius"));
    prefer_correct_edge_insets_constructor::PreferCorrectEdgeInsetsConstructor => ("prefer-correct-edge-insets-constructor", file, Some(Domain::Flutter), true, RuleSource::DartCodeLinter("prefer-correct-edge-insets-constructor"));
    prefer_declaring_const_constructor::PreferDeclaringConstConstructor => ("prefer-declaring-const-constructor", resolved, Some(Domain::Flutter), false, RuleSource::PyramidLint("prefer_declaring_const_constructor"));
    avoid_slow_collection_methods::AvoidSlowCollectionMethods => ("avoid-slow-collection-methods", resolved, None, false, RuleSource::Falcon);
    move_variable_outside_iteration::MoveVariableOutsideIteration => ("move-variable-outside-iteration", resolved, None, false, RuleSource::Falcon);
    avoid_shrink_wrap_in_lists::AvoidShrinkWrapInLists => ("avoid-shrink-wrap-in-lists", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    prefer_contains::PreferContains => ("prefer-contains", resolved, None, true, RuleSource::Lints("prefer_contains"));
    avoid_unnecessary_consumer_widgets::AvoidUnnecessaryConsumerWidgets => ("avoid-unnecessary-consumer-widgets", resolved, Some(Domain::Riverpod), false, RuleSource::Falcon);
    avoid_slow_async_io::AvoidSlowAsyncIo => ("avoid-slow-async-io", resolved, None, false, RuleSource::Lints("avoid_slow_async_io"));
    noop_primitive_operations::NoopPrimitiveOperations => ("noop-primitive-operations", resolved, None, true, RuleSource::Lints("noop_primitive_operations"));
    prefer_foreach::PreferForeach => ("prefer-foreach", resolved, None, false, RuleSource::Lints("prefer_foreach"));
    avoid_unnecessary_hook_widgets::AvoidUnnecessaryHookWidgets => ("avoid-unnecessary-hook-widgets", resolved, Some(Domain::FlutterHooks), false, RuleSource::Falcon);
    prefer_use_callback::PreferUseCallback => ("prefer-use-callback", resolved, Some(Domain::FlutterHooks), false, RuleSource::Falcon);
    prefer_use_memo::PreferUseMemo => ("prefer-use-memo", resolved, Some(Domain::FlutterHooks), false, RuleSource::Falcon);
    use_is_even_rather_than_modulo::UseIsEvenRatherThanModulo => ("use-is-even-rather-than-modulo", resolved, None, false, RuleSource::Lints("use_is_even_rather_than_modulo"));
    use_string_buffers::UseStringBuffers => ("use-string-buffers", resolved, None, true, RuleSource::Lints("use_string_buffers"));
    avoid_unnecessary_collections::AvoidUnnecessaryCollections => ("avoid-unnecessary-collections", resolved, None, false, RuleSource::Falcon);
    move_variable_closer_to_its_usage::MoveVariableCloserToItsUsage => ("move-variable-closer-to-its-usage", resolved, None, false, RuleSource::Falcon);
    avoid_unnecessary_futures::AvoidUnnecessaryFutures => ("avoid-unnecessary-futures", resolved, None, false, RuleSource::Falcon);
    prefer_return_await::PreferReturnAwait => ("prefer-return-await", resolved, None, false, RuleSource::Falcon);
}
