// Test cases for double-literal-format rule
// All violations are marked inline below.

// Missing leading zero: `.5` should be `0.5`.
void testMissingLeadingZero() {
  final opacity = .5; /* expect: double-literal-format */
  final scale = .75; /* expect: double-literal-format */
  final threshold = .001; /* expect: double-literal-format */
  final value = .999; /* expect: double-literal-format */
}

// Redundant trailing zero: the fractional part has a non-"0" digit, so the
// trailing zero is pure noise (`1.50` → `1.5`), and stripping it keeps a double.
void testRedundantTrailingZero() {
  const half = 1.50; /* expect: double-literal-format */
  const tenth = 0.50; /* expect: double-literal-format */
  final long = 1.230; /* expect: double-literal-format */
  final doubled = 1.00; /* expect: double-literal-format */
}

class Animation {
  final double speed = .5; /* expect: double-literal-format */
  final double curve = .25; /* expect: double-literal-format */
  final double trailing = 2.50; /* expect: double-literal-format */
}

double calculateOpacity() {
  return .8; /* expect: double-literal-format */
}

void processValues() {
  final list = [.1, .2, .3]; /* expect: double-literal-format */ /* expect: double-literal-format */ /* expect: double-literal-format */
  final map = {
    'x': .5, /* expect: double-literal-format */
    'y': 1.50, /* expect: double-literal-format */
  };
}

void mathOperations() {
  final result = .5 + .25; /* expect: double-literal-format */ /* expect: double-literal-format */
  final product = 2.50 * .75; /* expect: double-literal-format */ /* expect: double-literal-format */
}

bool validateRange(double value) {
  return value > .0; /* expect: double-literal-format */
}

void mixedFormats() {
  final a = .5; /* expect: double-literal-format */
  final c = .125; /* expect: double-literal-format */
}

// Regression: a literal must be found wherever it appears, including inside
// Dart 3 containers whose walkers were previously not visited.
void testContainers(List<double> xs, int count) {
  final (a, _) = (.5, 0); /* expect: double-literal-format */
  double b;
  int _u;
  (b, _u) = (1.50, 0); /* expect: double-literal-format */
  lbl: {
    final c = .25; /* expect: double-literal-format */
    print(c);
  }
  for (final (d, _) in [(.75, 0)]) { /* expect: double-literal-format */
    print(d);
  }
  final e = switch (count) {
    0 => .125, /* expect: double-literal-format */
    _ => 1.230, /* expect: double-literal-format */
  };
  final f = switch (.5) { /* expect: double-literal-format */
    _ => 0,
  };
  final g = [if (count > 0) .5]; /* expect: double-literal-format */
  final h = [...[.5]]; /* expect: double-literal-format */
  final i = (x: .5, y: 0); /* expect: double-literal-format */
  final j = {for (final k in xs) k: .5}; /* expect: double-literal-format */
  assert(.5 > 0, "x"); /* expect: double-literal-format */
  final l = (.5 as Object) is double; /* expect: double-literal-format */
  print([a, b, e, f, g, h, i, j, l]);
}
