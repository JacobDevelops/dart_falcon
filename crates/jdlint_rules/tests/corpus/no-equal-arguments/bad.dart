// Test cases for no-equal-arguments rule
// All violations are marked inline below.

void testEqualArguments() {
  foo(value, value); /* expect: no-equal-arguments */
  bar(x, x, z); /* expect: no-equal-arguments */
  baz(a, b, a); /* expect: no-equal-arguments */
}

void testRectFromPoints() {
  final rect = Rect.fromPoints(start, start); /* expect: no-equal-arguments */
}

void testStringOperations() {
  final result = str.replaceAll("a", "a"); /* expect: no-equal-arguments */
  final check = areEqual(value, value); /* expect: no-equal-arguments */
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
