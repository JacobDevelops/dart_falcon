import 'dart:collection';

void good(
  Iterable<num> iterable,
  List<String> list,
  Set<int> set,
  Queue<String> queue,
  Map<String, int> map,
  dynamic unknown,
) {
  iterable.contains(1);
  list.remove('x');
  set.lookup(1);
  queue.remove('x');
  map['key'];
  map.containsKey('key');
  map.containsValue(1);
  map.remove('key');
  map.containsKey(unknown);
}

class Map<K, V> {
  bool containsKey(Object? value) => false;
}

void shadowed(Map<String, int> map) {
  map.containsKey(1);
}
