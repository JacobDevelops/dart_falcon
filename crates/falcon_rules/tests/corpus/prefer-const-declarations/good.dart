import 'dart:math' as math;

const o = <int>[];

// Non-const collection literals create fresh mutable values.
final list = <int>[];
final map = {'a': 1};
final set = {1, 2};

// Only every declarator being constant qualifies.
final one = 1, now = DateTime.now();

// Instance fields are out of scope.
class A {
  static const o = <int>[];
  final instance = 1;
  static var mutable = 1;
  static final computed = DateTime.now();
}

var counter = 0;
final fromVariable = counter;

// Implicit `new` is not constant even for const constructors.
class Point {
  const Point(this.x, this.y);
  final int x;
  final int y;
}

final implicit = Point(0, 0);

// Imported or unknown names cannot be proven constant.
final pi = math.pi;
final unknown = somewhereElse;

late final lazy = DateTime.now();

void locals(int parameter) {
  const c = 1;
  final fromParam = parameter;
  final fromCall = c.toString();
  final typeTest = c is int;
  final noInit;
  noInit = 2;
  final interpolatedVar = 'value $parameter';
  print([fromParam, fromCall, typeTest, noInit, interpolatedVar]);
}

const shadowed = 1;

void shadowing(int shadowed) {
  // The parameter shadows the top-level constant.
  final copy = shadowed;
  print(copy);
}

void loops(List<int> values) {
  for (final value in values) {
    final copy = value;
    print(copy);
  }
}

void divide() {
  final bad = 1 ~/ 0;
  print(bad);
}

final nonConstantTuple = (1, () {});
