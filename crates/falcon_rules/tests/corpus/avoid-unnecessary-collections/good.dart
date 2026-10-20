import 'dart:async';
class Custom {
  void addAll(Object value) {}
  bool containsAll(Object value) => true;
}
void append(List<int> list, Set<int> set, Map<int, int> map,
    Iterable<int> source, dynamic unknown, bool condition) {
  list.addAll([]);
  list.addAll([1, 2]);
  list.addAll([...source]);
  list.addAll([if (condition) 1]);
  list.addAll([for (final value in source) value]);
  set.containsAll(source);
  map.addAll({1: 2});
  unknown.addAll([1]);
  Custom().addAll([1]);
  Custom().containsAll({1});
}
Future<List<int>> used(Future<int> value) async {
  return await Future.wait([value]);
}
Future<void> options(Future<int> value) async {
  await Future.wait([value], eagerError: true);
  await Future.wait([value], cleanUp: (value) {});
}
Stream<int> multiple(Future<int> first, Future<int> second) => Stream.fromFutures([first, second]);
void shadow(dynamic Future, dynamic Stream) {
  Future.any([1]);
  Stream.fromIterable([1]);
}
void discarded(Future<int> value) {
  Future.any([value]);
  (Future.any([value]));
  Future.any([value])..ignore();
}
void discardedArrow(Future<int> value) => Future.any([value]);
