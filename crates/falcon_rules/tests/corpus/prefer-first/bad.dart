// Bad: using [0] instead of .first
void example() {
  final items = [1, 2, 3];
  final first = items[0]; /* expect: prefer-first */
  print(first);
}

class Processor {
  String getFirstName(List<String> names) {
    return names[0]; /* expect: prefer-first */
  }

  void processHead(List<int> values) {
    final head = values[0]; /* expect: prefer-first */
    if (head > 0) {
      compute(head);
    }
  }
}

void multipleViolations(List<String> items) {
  final a = items[0]; /* expect: prefer-first */
  final b = items[0].length; /* expect: prefer-first */
  print('$a $b');
}

// Regression: the violation must be found inside Dart 3 containers too.
void containers(List<int> xs, int count) {
  final (a, _) = (xs[0], 0); /* expect: prefer-first */
  int b;
  int _u;
  (b, _u) = (xs[0], 0); /* expect: prefer-first */
  lbl: {
    final c = xs[0]; /* expect: prefer-first */
    print(c);
  }
  final d = switch (count) {
    0 => xs[0], /* expect: prefer-first */
    _ => 0,
  };
  final e = switch (xs[0]) { /* expect: prefer-first */
    _ => 0,
  };
  final f = [if (count > 0) xs[0]]; /* expect: prefer-first */
  final g = [...[xs[0]]]; /* expect: prefer-first */
  final h = (p: xs[0], q: 0); /* expect: prefer-first */
  assert(xs[0] > 0, "x"); /* expect: prefer-first */
  print([a, b, d, e, f, g, h]);
}
