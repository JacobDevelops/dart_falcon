void read((int, String) record, (int, String)? nullable) {
  print(record.$1); /* expect: avoid-positional-fields-in-records */
  print(record.$2); /* expect: avoid-positional-fields-in-records */
  print((record).$1); /* expect: avoid-positional-fields-in-records */
  print(nullable?.$2); /* expect: avoid-positional-fields-in-records */
  final inferred = (1, 2);
  print(inferred.$1); /* expect: avoid-positional-fields-in-records */
  print((3, 4).$2); /* expect: avoid-positional-fields-in-records */
}
