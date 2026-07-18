void good() {
  items.forEach(print);
  list?.forEach((e) => print(e));
  map.forEach((k, v) => print('$k$v'));
  for (final e in items) {
    print(e);
  }
  numbers.map((e) => e * 2);
}
