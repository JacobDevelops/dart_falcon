import 'package:flutter_riverpod/flutter_riverpod.dart' as rp;

String whenCall(rp.AsyncValue<int> v) => v.when(
  loading: () => 'loading', error: (error, stack) => 'error', data: (value) => 'data');
Object patterns(rp.AsyncValue<int> v) => switch (v) {
  rp.AsyncData(:final value) => value,
  rp.AsyncError() => 'error',
  _ => 'loading',
};
void isolated(rp.AsyncValue<int> v) {
  if (v.isLoading) return;
}
String different(rp.AsyncValue<int> a, rp.AsyncValue<int> b) {
  if (a.isLoading) return 'loading';
  else if (b.hasError) return 'error';
  return 'data';
}
class AsyncValue {
  bool get isLoading => false;
  bool get hasError => false;
}
String lookalike(AsyncValue v) => v.isLoading ? 'loading' : v.hasError ? 'error' : 'data';
class Flags {
  bool get busy => false;
  bool get failed => false;
}
String unrelated(Flags v) => v.busy ? 'loading' : v.failed ? 'error' : 'data';
String compound(rp.AsyncValue<int> v, bool ready) {
  if (v.isLoading && ready) return 'loading';
  else if (v.hasError) return 'error';
  return 'data';
}
String repeated(rp.AsyncValue<int> v) => v.isLoading ? 'one' : !v.isLoading ? 'two' : 'three';
String unconfirmed(dynamic v) => v.isLoading ? 'loading' : v.hasError ? 'error' : 'data';
String mixed(rp.AsyncValue<int> v, dynamic other) {
  if (v.isLoading) return 'loading';
  else if (v.hasError) return 'error';
  else if (other.hasValue) return 'other';
  return 'data';
}
class Holder<T> {
  final rp.AsyncValue<T> value;
  Holder(this.value);
}
String genericDifferent(Holder<int> first, Holder<int> second) =>
    first.value.isLoading ? 'loading' : second.value.hasError ? 'error' : 'data';
String unrelatedHead(rp.AsyncValue<int> v, bool flag) {
  if (flag) return 'other';
  else if (v.isLoading) return 'loading';
  else if (v.hasError) return 'error';
  return 'data';
}
String compoundEquality(rp.AsyncValue<int>? v) =>
    v?.isLoading == true ? 'loading' : v?.hasError == true ? 'error' : 'data';
String caseCondition(rp.AsyncValue<int> v) {
  if (v case rp.AsyncLoading<int>()) return 'loading';
  else if (v.hasError) return 'error';
  return 'data';
}
List<String> isolatedCollection(rp.AsyncValue<int> v) => [
  if (v.isLoading) 'loading',
];
Map<String, String> differentCollection(rp.AsyncValue<int> a, rp.AsyncValue<int> b) => {
  if (a.isLoading) 'state': 'loading' else if (b.hasError) 'state': 'error',
};
List<String> caseCollection(rp.AsyncValue<int> v) => [
  if (v case rp.AsyncLoading<int>()) 'loading' else if (v.hasError) 'error',
];
List<String> unrelatedCollectionHead(rp.AsyncValue<int> v, bool flag) => [
  if (flag) 'other' else if (v.isLoading) 'loading' else if (v.hasError) 'error',
];
List<String> collectionCaseTail(rp.AsyncValue<int> v) => [
  if (v.isLoading) 'loading' else if (v.hasError) 'error' else if (v case rp.AsyncData<int>()) 'data',
];
String boundedLookalike<T extends AsyncValue>(T v) =>
    v.isLoading ? 'loading' : v.hasError ? 'error' : 'data';
