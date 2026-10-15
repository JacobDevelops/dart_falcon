enum First { a }
enum Second { b }

void inferredReceivers(Map<String, int>? map) {
  (map ?? {}).containsKey(1); /* expect: collection-methods-unrelated-type */
  [for (var value in [1]) value].contains('x'); /* expect: collection-methods-unrelated-type */
  <First>[].contains(Second.b); /* expect: collection-methods-unrelated-type */
}

T pick<T>(T first, T second) => first;
T one<T>(T value) => value;

void contextualArguments(List<int> values, Set<int> set, Map<int, int> map) {
  values.contains(pick('a', 'b'));
  values.remove(one('a'));
  set.lookup(pick('a', 'b'));
  set.remove(one('a'));
  map.containsKey(pick('a', 'b'));
  map.containsValue(one('a'));
  map.remove(one('a'));
  map[one('a')];
  values..contains(one('a'));
  values.contains(pick<String>('a', 'b')); /* expect: collection-methods-unrelated-type */
  map[one<String>('a')]; /* expect: collection-methods-unrelated-type */
}
