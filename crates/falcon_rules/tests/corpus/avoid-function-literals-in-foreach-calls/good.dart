void good(List<int> items, List<int>? list, Map<int, int> map, List<int> numbers) {
  items.forEach(print);
  list?.forEach((e) => print(e));
  map.forEach((k, v) => print('$k$v'));
  for (final e in items) {
    print(e);
  }
  numbers.map((e) => e * 2);
}
