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
