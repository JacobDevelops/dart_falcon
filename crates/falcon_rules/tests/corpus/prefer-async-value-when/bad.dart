import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

String getters(AsyncValue<int> v) {
  if (v.isLoading) { /* expect: prefer-async-value-when, msg: "Use `AsyncValue.when` instead of branching on loading, error and data states manually." */
    return 'loading';
  } else if (v.hasError) {
    return 'error';
  } else {
    return 'data';
  }
}
String ternary(AsyncValue<int> v) =>
    v.isLoading ? 'loading' : v.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
String types(AsyncValue<int> v) {
  if (v is AsyncLoading<int>) { /* expect: prefer-async-value-when */
    return 'loading';
  } else if (v is AsyncError<int>) {
    return 'error';
  }
  return 'data';
}
String negated(hooks.AsyncValue<int> v) {
  if (!v.hasValue) { /* expect: prefer-async-value-when */
    return 'empty';
  } else if (v.hasError) {
    return 'error';
  }
  return 'data';
}
class Screen {
  final AsyncValue<int> value;
  Screen(this.value);
  String render() {
    if (this.value.isLoading) { /* expect: prefer-async-value-when */
      return 'loading';
    } else if (value.hasError) {
      return 'error';
    } else if (value.hasValue) {
      return 'data';
    }
    return 'empty';
  }
}
String dataTest(AsyncValue<int> v) =>
    v is AsyncData<int> ? 'data' : v is AsyncLoading<int> ? 'loading' : 'error'; /* expect: prefer-async-value-when */
class Holder<T> {
  final AsyncValue<T> value;
  Holder(this.value);
}
String genericField(Holder<int> holder) =>
    holder.value.isLoading ? 'loading' : holder.value.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
class GenericScreen<T> {
  final AsyncValue<T> value;
  GenericScreen(this.value);
  String render() =>
      value.isLoading ? 'loading' : value.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
}
class State<T> {
  final T widget;
  State(this.widget);
}
class GenericState extends State<Holder<int>> {
  GenericState(super.widget);
  String render() =>
      widget.value.isLoading ? 'loading' : widget.value.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
}
List<String> collection(AsyncValue<int> v) => [
  if (v.isLoading) 'loading' else if (v.hasError) 'error' else 'data', /* expect: prefer-async-value-when */
];
Map<String, String> mapCollection(AsyncValue<int> v) => {
  if (v.isLoading) 'state': 'loading' else if (v.hasError) 'state': 'error' else 'state': 'data', /* expect: prefer-async-value-when */
};
String bounded<T extends AsyncValue<int>>(T v) =>
    v.isLoading ? 'loading' : v.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
Set<String> nestedCollection(AsyncValue<int> v) => {
  for (final i in [1]) if (v.isLoading) 'loading' else if (v.hasError) 'error' else 'data', /* expect: prefer-async-value-when */
};
List<String> spreadCollection(AsyncValue<int> v) => [
  ...[if (v.isLoading) 'loading' else if (v.hasError) 'error' else 'data'], /* expect: prefer-async-value-when */
];
String indirectBound<T extends AsyncValue<int>, U extends T>(U v) =>
    v.isLoading ? 'loading' : v.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
String promotedBound<T extends AsyncValue<int>?>(T v) {
  if (v != null) {
    return v.isLoading ? 'loading' : v.hasError ? 'error' : 'data'; /* expect: prefer-async-value-when */
  }
  return 'absent';
}
