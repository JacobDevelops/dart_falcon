// Good examples for avoid-unnecessary-type-assertions rule
// Type checks where the type is not already known

void testDynamicTypeCheck() {
  dynamic value = 42;
  if (value is int) {
    print("value is int");
  }
}

void testObjectTypeCheck() {
  Object obj = "hello";
  if (obj is String) {
    print("obj is string");
  }
}

void testParameterTypeCheck() {
  void checkType(dynamic param) {
    if (param is bool) {
      print("param is bool");
    }
  }
}

void testNullableTypeCheck() {
  int? maybeInt = 5;
  if (maybeInt is int) {
    print("maybeInt is int");
  }
}

void testBroadTypeCheck() {
  List<dynamic> items = [];
  if (items is List<String>) {
    print("items is List<String>");
  }
}

void testInheritanceTypeCheck() {
  Object shape = Circle();
  if (shape is Circle) {
    print("shape is Circle");
  }
}

class Circle {}
