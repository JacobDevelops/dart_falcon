class A {}

class B extends A {}

String? maybeString() => null;

A? maybeA() => null;

void f(String s, String? n, dynamic d, Object o) {
  var a = n!;
  var b = s as String;
  var c = n as String?;
  var e = d as String;
  var g = o as String;
  var h = maybeA() as B?;
  var i = maybeString() as dynamic;
  var j = maybeString() as Object?;
  print([a, b, c, e, g, h, i, j]);
}

void promoted(String? s) {
  if (s != null) {
    print(s as String);
  }
}

T generic<T>(Object? value) => value as T;

void initialized() {
  int? maybeInt = 5;
  final int nonNull = maybeInt as int;
  print(nonNull);
}
