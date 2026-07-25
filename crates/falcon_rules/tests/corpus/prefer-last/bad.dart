// Bad: using [list.length - 1] instead of .last
void example() {
  final items = [1, 2, 3];
  final last = items[items.length - 1]; /* expect: prefer-last */
  print(last);
}

class Processor {
  String getLastName(List<String> names) {
    return names[names.length - 1]; /* expect: prefer-last */
  }

  void processTail(List<int> values) {
    final tail = values[values.length - 1]; /* expect: prefer-last */
    if (tail > 0) {
      compute(tail);
    }
  }
}

void multipleViolations(List<String> items) {
  final a = items[items.length - 1]; /* expect: prefer-last */
  final b = items[items.length - 1].length; /* expect: prefer-last */
  print('$a $b');
}

// Regression: the violation must be found inside Dart 3 containers too.
void containers(List<int> xs, int count) {
  final (a, _) = (xs[xs.length - 1], 0); /* expect: prefer-last */
  int b;
  int _u;
  (b, _u) = (xs[xs.length - 1], 0); /* expect: prefer-last */
  lbl: {
    final c = xs[xs.length - 1]; /* expect: prefer-last */
    print(c);
  }
  final d = switch (count) {
    0 => xs[xs.length - 1], /* expect: prefer-last */
    _ => 0,
  };
  final e = switch (xs[xs.length - 1]) { /* expect: prefer-last */
    _ => 0,
  };
  final f = [if (count > 0) xs[xs.length - 1]]; /* expect: prefer-last */
  final g = [...[xs[xs.length - 1]]]; /* expect: prefer-last */
  final h = (p: xs[xs.length - 1], q: 0); /* expect: prefer-last */
  assert(xs[xs.length - 1] > 0, "x"); /* expect: prefer-last */
  print([a, b, d, e, f, g, h]);
}
