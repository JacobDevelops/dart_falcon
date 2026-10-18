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
  await Future.wait([value]); /* expect: avoid-unnecessary-collections */
  await async.Future.any([value]); /* expect: avoid-unnecessary-collections */
}
Stream<int> fromFuture(Future<int> value) => Stream.fromFutures([value]); /* expect: avoid-unnecessary-collections */
Stream<int> fromValue(int value) => async.Stream.fromIterable([value]); /* expect: avoid-unnecessary-collections */
