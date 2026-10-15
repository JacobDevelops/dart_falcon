import 'dart:collection';
import 'dart:core' as core;

void good(
  Iterable<num> iterable,
  List<String> list,
  Set<int> set,
  Queue<String> queue,
  core.Map<String, int> map,
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

void functionKeys(core.Map<core.Function?, core.int> makers, core.int Function() maker) {
  makers[maker];
}

// Named record fields match by name, not by source order.
void recordKeys() {
  final byScope = <({core.String id, core.int count}), core.List<core.int>>{};
  final key = (id: 'x', count: 1);
  (byScope[key] ??= <core.int>[]).add(1);
  byScope.containsKey((id: 'y', count: 2));
}
