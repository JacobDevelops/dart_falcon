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

void typeTests() {
  const c = 1;
  final typeTest = c is int; /* expect: prefer-const-declarations */
  final cast = c as num; /* expect: prefer-const-declarations */
  final interpolatesConstant = 'c=$c'; /* expect: prefer-const-declarations */
  final length = 'abc'.length; /* expect: prefer-const-declarations */
  print([typeTest, cast, interpolatesConstant, length]);
}

void forBodies() {
  for (final i = 0; i < 1;) {
    final body = 1; /* expect: prefer-const-declarations */
    print(body);
    break;
  }
  for (final callback = () {
    final nested = 1; /* expect: prefer-const-declarations */
    return nested;
  }; callback() > 0;) {
    break;
  }
}

int topLevelFunction() => 0;

class Factory {
  const Factory.origin();
  static int create() => 0;
  static const prefix = 'p';
  static final prefixed = prefix + 'q'; /* expect: prefer-const-declarations */
}

final functionTearOff = topLevelFunction; /* expect: prefer-const-declarations */
final staticTearOff = Factory.create; /* expect: prefer-const-declarations */
final typeLiteral = Factory; /* expect: prefer-const-declarations */
final colors = Color.values; /* expect: prefer-const-declarations */
final Factory dotShorthand = const .origin(); /* expect: prefer-const-declarations */
final equality = 1 == 2.0; /* expect: prefer-const-declarations */
final coalesced = null ?? 'fallback'; /* expect: prefer-const-declarations */
final numericMix = 1 + 2.5 * base - 7 ~/ 2; /* expect: prefer-const-declarations */
final bits = (1 << 3) & 0xff | (true ^ false ? 1 : 0); /* expect: prefer-const-declarations */
