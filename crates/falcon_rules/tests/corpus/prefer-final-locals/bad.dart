class Point {
  final int x;
  final int y;
  Point(this.x, this.y);
}

int compute(List<int> items) {
  var total = items.length; /* expect: prefer-final-locals */
  int doubled = total * 2; /* expect: prefer-final-locals */
  var a = 1, b = 2; /* expect: prefer-final-locals */
  late var lazy = a + b; /* expect: prefer-final-locals */
  return doubled + lazy;
}

void patterns((int, int) pair, Object o, List<(int, String)> rows) {
  var (left, right) = pair; /* expect: prefer-final-locals */
  print(left + right);
  for (var (n, s) in rows) { /* expect: prefer-final-locals */ /* expect: prefer-final-locals */
    print('$n$s');
  }
  if (o case int value) { /* expect: prefer-final-locals */
    print(value);
  }
  switch (o) {
    case String text: /* expect: prefer-final-locals */
      print(text);
    case Point(x: var px): /* expect: prefer-final-locals */
      print(px);
  }
}

void closures() {
  var shadowed = 1; /* expect: prefer-final-locals */
  void inner(int shadowed) {
    shadowed = 2;
    print(shadowed);
  }

  inner(shadowed);
  for (var i = 0; i < 3;) { /* expect: prefer-final-locals */
    break;
  }
}
