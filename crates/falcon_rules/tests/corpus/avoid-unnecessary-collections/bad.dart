import 'dart:async';
import 'dart:async' as async;

void append(List<int> list, Set<int> set) {
  list.addAll([1]); /* expect: avoid-unnecessary-collections */
  list.addAll({2}); /* expect: avoid-unnecessary-collections */
  set.addAll([3]); /* expect: avoid-unnecessary-collections */
  set.addAll({4}); /* expect: avoid-unnecessary-collections */
  set.containsAll({5}); /* expect: avoid-unnecessary-collections */
}
Future<void> futures(Future<int> value) async {
  await Future.wait([value]); /* expect: avoid-unnecessary-collections, msg: "Use the single `Future` directly instead of wrapping it in `Future.wait` or `Future.any`." */
  await async.Future.any([value]); /* expect: avoid-unnecessary-collections, msg: "Use the single `Future` directly instead of wrapping it in `Future.wait` or `Future.any`." */
}
Stream<int> fromFuture(Future<int> value) => Stream.fromFutures([value]); /* expect: avoid-unnecessary-collections */
Stream<int> fromValue(int value) => async.Stream.fromIterable([value]); /* expect: avoid-unnecessary-collections */
Future<int> first(Future<int> value) {
  return Future.any([value]); /* expect: avoid-unnecessary-collections, msg: "Use the single `Future` directly instead of wrapping it in `Future.wait` or `Future.any`." */
}
Future<int> assigned(Future<int> value) {
  final result = Future.any([value]); /* expect: avoid-unnecessary-collections */
  return result;
}
void awaitedArrow(Future<int> value) async => Future.any([value]); /* expect: avoid-unnecessary-collections */
void consumedSwitch(Future<int> value) => switch (Future.any([value])) { /* expect: avoid-unnecessary-collections */
  final result => result.ignore(),
};
