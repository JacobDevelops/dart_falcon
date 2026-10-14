final int topLevel = 1;

class Holder {
  final int value;
  static final Holder empty = Holder(0);

  Holder(this.value);
}

void params(int x, {String? name}) {
  var y = x + 1;
  int z = y;
  const w = 3;
  print('$z$name$w');
}

void loops(List<int> numbers, List<(int, int)> rows) {
  for (var n in numbers) {
    print(n);
  }
  for (var (a, b) in rows) {
    print(a + b);
  }
  var (c, d) = rows.first;
  print(c + d);
  for (final int i = 0; i < 1;) {
    print(i);
    break;
  }
}

void cases(Object o) {
  if (o case int value) {
    print(value);
  }
  switch (o) {
    case var other:
      print(other);
  }
}
