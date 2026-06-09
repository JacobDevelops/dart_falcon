// Good examples for avoid-unnecessary-type-casts rule
// Type casts where the variable is not already known to be T

void testCastDynamicToInt() {
  dynamic value = 42;
  final int x = value as int;
}

void testCastObjectToString() {
  Object obj = "hello";
  final String name = obj as String;
}

void testCastFromParameter() {
  void process(dynamic param) {
    final int x = param as int;
    print(x);
  }
}

void testCastNullableToNonNullable() {
  int? maybeInt = 5;
  final int nonNull = maybeInt as int;
}

void testCastBroadToNarrow() {
  List<dynamic> items = [1, 2, 3];
  final List<int> ints = items as List<int>;
}

void testCastBetweenUnrelatedTypes() {
  Object shape = Circle();
  final Circle c = shape as Circle;
}

void testCastFromJsonDecode() {
  dynamic json = {"id": 42};
  final Map<String, dynamic> data = json as Map<String, dynamic>;
}

class Circle {}
