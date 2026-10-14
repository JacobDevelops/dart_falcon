class Pair {
  final int a;
  Pair(this.a);
}

void params(final int x, {final String? name}) { /* expect: unnecessary-final */ /* expect: unnecessary-final */
  final y = x + 1; /* expect: unnecessary-final */
  final int z = y; /* expect: unnecessary-final */
  print('$z$name');
}

void loops(List<int> numbers, List<(int, int)> rows) {
  for (final n in numbers) { /* expect: unnecessary-final */
    print(n);
  }
  for (final (a, b) in rows) { /* expect: unnecessary-final */
    print(a + b);
  }
  final (c, d) = rows.first; /* expect: unnecessary-final */
  print(c + d);
}

void cases(Object o) {
  if (o case final int value) { /* expect: unnecessary-final */
    print(value);
  }
  switch (o) {
    case Pair(a: final first): /* expect: unnecessary-final */
      print(first);
  }
  final handler = (final int v) => v; /* expect: unnecessary-final */ /* expect: unnecessary-final */
  print(handler);
}
