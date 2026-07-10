// Good cases for avoid-dynamic rule
// No violations expected

void testTypedVariable() {
  int x = 5;
  String name = "test";
  List<int> list = [1, 2, 3];
}

void testTypedParameters(int arg, String other) {
  print(arg);
  print(other);
}

Future<int> asyncMethod() {
  return Future.value(42);
}

String returnString() {
  return "anything";
}

class TestClass {
  Object field = Object();

  void method(Object param) {
    final Object local = param;
  }
}

Map<String, int> mapWithType = {};

List<String> listOfString = [];

void testWithGenerics<T>(T value) {
  print(value);
}

void testWithUnion(Object? nullable) {
  if (nullable != null) {
    print(nullable);
  }
}

// dcl exempts `dynamic` used directly as a `Map` type argument (the JSON escape
// hatch) — both in a `Map<...>` annotation and in a `<...>{}` map literal.
Map<String, dynamic> jsonField = {};

Map<String, dynamic> parseJson(Map<String, dynamic> json) {
  final literal = <String, dynamic>{'k': 1};
  return literal;
}

class Serializable {
  Map<String, dynamic> toJson() => <String, dynamic>{};
}
