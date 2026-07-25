// Test cases for no-equal-arguments rule
// All violations are marked inline below.

void testEqualArguments() {
  foo(value, value); /* expect: no-equal-arguments */
  bar(x, x, z); /* expect: no-equal-arguments */
  baz(a, b, a); /* expect: no-equal-arguments */
}

// A label on the enclosing loop must not hide the equal-argument call inside.
void testLabeledLoop() {
  outer:
  for (var i = 0; i < 3; i++) {
    foo(value, value); /* expect: no-equal-arguments */
  }
}

void testRectFromPoints() {
  final rect = Rect.fromPoints(start, start); /* expect: no-equal-arguments */
}

void testStringOperations() {
  final check = areEqual(value, value); /* expect: no-equal-arguments */
}

// dcl reports on the LAST occurrence of the duplicate, so a hand-written
// `// ignore` on the trailing argument lines up. The annotation therefore
// belongs on the final `padding` argument, not the first.
void testLastOccurrenceLocation() {
  const value = EdgeInsets.fromLTRB(
    padding,
    other,
    padding,
    padding, /* expect: no-equal-arguments */
  );
}

class Math {
  static double min(double a, double b) => a < b ? a : b;

  static int gcd(int a, int b) {
    if (a == b) return a;
    return b == 0 ? a : gcd(b, a % b);
  }
}

void testDuplicateNamed() {
  createUser(
    name: userName,
    email: userName, /* expect: no-equal-arguments */
  );
}

bool compare(int x, int y) {
  return equals(x, x); /* expect: no-equal-arguments */
}

void testListOperations() {
  final list = [1, 2, 3];
  list.setRange(0, 2, list);
}

void setupAnimation(Animation anim) {
  anim.addListener(anim.forward);
}

void copyMap(Map map) {
  final copy = Map.from(map);
  map.addAll(map);
}

void testMultipleDuplicates(String a, String b) {
  process(a, a, b); /* expect: no-equal-arguments */
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, labeled statement, switch expression and subject,
// collection if/spread, record field).
void containersRegression(int rcount) {
  final (ra, _) = (foo(rcount, rcount), 0); /* expect: no-equal-arguments */
  lbl: {
    final rb = foo(rcount, rcount); /* expect: no-equal-arguments */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => foo(rcount, rcount), /* expect: no-equal-arguments */
    _ => null,
  };
  final rd = switch (foo(rcount, rcount)) { /* expect: no-equal-arguments */
    _ => 0,
  };
  final re = [if (rcount > 0) foo(rcount, rcount)]; /* expect: no-equal-arguments */
  final rf = [...[foo(rcount, rcount)]]; /* expect: no-equal-arguments */
  final rg = (p: foo(rcount, rcount), q: 0); /* expect: no-equal-arguments */
  print([ra, rc, rd, re, rf, rg]);
}

// Regression: a cascade call carries its own argument list and is never
// rebuilt as a plain call expression, so it needs checking on its own.
class Configurable {
  void configure(int a, int b) {}
}

void cascadeRegression(Configurable c, int dup) {
  c
    ..configure(dup, dup) /* expect: no-equal-arguments */
    ..configure(1, 2);
}
