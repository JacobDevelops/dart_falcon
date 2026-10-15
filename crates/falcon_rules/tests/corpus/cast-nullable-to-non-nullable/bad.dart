class A {}

class B extends A {}

String? maybeString() => null;

int? maybeInt() => null;

A? maybeA() => null;

void f(String? s, int? count, List<int>? list) {
  var a = s as String; /* expect: cast-nullable-to-non-nullable */
  var b = count as int; /* expect: cast-nullable-to-non-nullable */
  var c = list as List<int>; /* expect: cast-nullable-to-non-nullable */
  var d = maybeString() as String; /* expect: cast-nullable-to-non-nullable */
  var e = maybeA() as B; /* expect: cast-nullable-to-non-nullable */
  var g = maybeInt() as num; /* expect: cast-nullable-to-non-nullable */
  print([a, b, c, d, e, g]);
}

Object g(A? input) => input as Object; /* expect: cast-nullable-to-non-nullable */

// Promoting sites only guard uses within the same declaration.
void checksElsewhere(Object? val) {
  if (val == null) return;
  print(val);
}

void castHere(Object? val) {
  print(val as String); /* expect: cast-nullable-to-non-nullable */
}

// Comparing with a non-null value does not promote.
void comparedToValue(Object? s) {
  if (s == 'none') return;
  print(s as String); /* expect: cast-nullable-to-non-nullable */
}

void untypedInitializer(int? p) {
  var copy = p;
  print(copy as int); /* expect: cast-nullable-to-non-nullable */
}

class Holder {
  final int? _value;
  Holder(this._value);

  int read() => _value as int; /* expect: cast-nullable-to-non-nullable */

  int checked() {
    if (_value == null) return 0;
    return _value;
  }
}
