bool positive(int value) => value > 0;

void search(Iterable<int> values, List<int> list, Set<int> set) {
  values.where(positive).first; /* expect: avoid-slow-collection-methods */
  values.where(positive).last; /* expect: avoid-slow-collection-methods */
  values.where(positive).single; /* expect: avoid-slow-collection-methods */
  list.where((value) => value > 2).first; /* expect: avoid-slow-collection-methods */
  set.where((value) => value.isEven).last; /* expect: avoid-slow-collection-methods */
  (values.where((value) => value < 10)).single; /* expect: avoid-slow-collection-methods */
  <int>[1, 2, 3].where(positive).first; /* expect: avoid-slow-collection-methods */
  values.map((value) => value + 1).where(positive).first; /* expect: avoid-slow-collection-methods */
}

void typeFilters(Iterable<Object> values, Object other) {
  values.where((value) => value is! String).first; /* expect: avoid-slow-collection-methods */
  values.where((value) { return value is String; }).first; /* expect: avoid-slow-collection-methods */
  values.where((value) => other is String).first; /* expect: avoid-slow-collection-methods */
}
