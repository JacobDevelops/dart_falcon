void emptyCollectionTypes() {
  final values = [];
  print(values.first.length); /* expect: avoid-dynamic-calls */
  print(values[0].length); /* expect: avoid-dynamic-calls */
  final entries = {};
  print(entries['a'].length); /* expect: avoid-dynamic-calls */
  final names = <dynamic>{};
  print(names.first.length); /* expect: avoid-dynamic-calls */
}

void dynamicCollectionTypes(dynamic value) {
  final values = [value, 1];
  print(values.first.length); /* expect: avoid-dynamic-calls */
  final entries = {'a': value};
  print(entries['a'].length); /* expect: avoid-dynamic-calls */
}
