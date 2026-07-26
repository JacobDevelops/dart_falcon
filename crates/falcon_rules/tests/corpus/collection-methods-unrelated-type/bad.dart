import 'dart:collection';

void bad(
  Iterable<int> iterable,
  List<String> list,
  Set<int> set,
  Queue<String> queue,
  Map<String, int> map,
) {
  iterable.contains('x'); /* expect: collection-methods-unrelated-type */
  list.remove(1); /* expect: collection-methods-unrelated-type */
  set.lookup('x'); /* expect: collection-methods-unrelated-type */
  set.remove(false); /* expect: collection-methods-unrelated-type */
  queue.remove(1); /* expect: collection-methods-unrelated-type */
  map['key'];
  map[1]; /* expect: collection-methods-unrelated-type */
  map.containsKey(1); /* expect: collection-methods-unrelated-type */
  map.containsValue('value'); /* expect: collection-methods-unrelated-type */
  map.remove(false); /* expect: collection-methods-unrelated-type */
}

class IntValues {
  final List<int> values;

  IntValues(this.values) {
    values.contains('x'); /* expect: collection-methods-unrelated-type */
  }
}

class BaseValues {
  final List<int> values;

  BaseValues(this.values);
}

class ForwardedValues extends BaseValues {
  ForwardedValues(super.values) {
    values.contains('x'); /* expect: collection-methods-unrelated-type */
  }
}

void scopedCollections(List<int> values) {
  <String>['x'].forEach((String value) {
    values.contains(value); /* expect: collection-methods-unrelated-type */
  });

  for (final value in <String>['x']) {
    values.remove(value); /* expect: collection-methods-unrelated-type */
  }

  for (final [value] in <List<String>>[
    ['x'],
  ]) {
    values.contains(value); /* expect: collection-methods-unrelated-type */
  }

  {
    final value = 'x';
    values.contains(value); /* expect: collection-methods-unrelated-type */
  }
  values.contains(1);
}
