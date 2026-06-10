// Good cases for no-equal-arguments rule
// No violations expected

void testDistinctArguments() {
  foo(value1, value2);
  bar(x, y, z);
  baz(a, b, c);
}

void testRectFromPoints() {
  final rect = Rect.fromPoints(topLeft, bottomRight);
}

void testStringOperations() {
  final result = str.replaceAll("old", "new");
  final check = areEqual(value1, value2);
}

class Math {
  static double min(double a, double b) => a < b ? a : b;

  static int gcd(int a, int b) {
    if (a == b) return a;
    return b == 0 ? a : gcd(b, a % b);
  }
}

void testDifferentNamed() {
  createUser(
    name: userName,
    email: userEmail,
  );
}

bool compare(int x, int y) {
  return equals(x, y);
}

void testListOperations() {
  final list = [1, 2, 3];
  final source = [4, 5, 6];
  list.setRange(0, 2, source);
}

void setupAnimation(Animation anim) {
  anim.addListener(() => anim.forward());
}

void copyMap(Map original) {
  final copy = Map.from(original);
  final updates = {'key': 'value'};
  copy.addAll(updates);
}

void testDistinctValues(String a, String b) {
  process(a, b, c, d);
}

void testWithDefaults(int value, {int other = 10}) {
  calculate(value, other);
}

void testFunctionCalls() {
  final x = getValue();
  final y = getOtherValue();
  combine(x, y);
}

class Pair<T> {
  final T first;
  final T second;

  Pair(this.first, this.second);

  bool areSame() => first == second;
  bool areDifferent() => first != second;
}

void testComparing(String a, String b) {
  if (a != b) {
    print('Different');
  }
}

void mergeCollections(List<int> list1, List<int> list2) {
  list1.addAll(list2);
}
