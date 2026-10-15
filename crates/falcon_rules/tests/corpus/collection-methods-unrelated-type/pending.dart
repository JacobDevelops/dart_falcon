import 'dart:collection';

enum First { a }
enum Second { b }

void pending(Map<String, int>? map, UnmodifiableListView<int> view) {
  view.contains('x'); /* expect: collection-methods-unrelated-type */
  (map ?? {}).containsKey(1); /* expect: collection-methods-unrelated-type */
  [for (var value in [1]) value].contains('x'); /* expect: collection-methods-unrelated-type */
  <First>[].contains(Second.b); /* expect: collection-methods-unrelated-type */
}
