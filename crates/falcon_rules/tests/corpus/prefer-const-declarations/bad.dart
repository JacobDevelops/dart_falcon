final o = const <int>[]; /* expect: prefer-const-declarations */

const base = 2;
final derived = base * 21; /* expect: prefer-const-declarations */

final String greeting = 'hello', other = 'world'; /* expect: prefer-const-declarations */

final Map<String, int> lookup = const {'a': 1}; /* expect: prefer-const-declarations */

enum Color { red, green }

final favorite = Color.red; /* expect: prefer-const-declarations */

class A {
  static final o = const <int>[]; /* expect: prefer-const-declarations */

  static const limit = 10;
  static final doubled = limit * 2; /* expect: prefer-const-declarations */

  static final interpolated = 'n=${1 + 2}'; /* expect: prefer-const-declarations */
}

class Point {
  const Point(this.x, this.y);
  final int x;
  final int y;
}

final origin = const Point(0, 0); /* expect: prefer-const-declarations */

void locals() {
  final a = 1; /* expect: prefer-const-declarations */
  final flag = !true && 3 > 2 ? null : -1; /* expect: prefer-const-declarations */
  const step = 5;
  final next = step + 1; /* expect: prefer-const-declarations */
  final sym = #name; /* expect: prefer-const-declarations */
  final viaClass = A.limit; /* expect: prefer-const-declarations */
  print([a, flag, next, sym, viaClass]);
}

final tuple = const ("first", 2, true); /* expect: prefer-const-declarations */
final implicitTuple = (number: 123, name: "Main"); /* expect: prefer-const-declarations */
