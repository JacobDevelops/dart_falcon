// Good examples for avoid-unrelated-type-assertions rule
// Type checks where the assertion is possible or plausible

void testDynamicTypeCheck() {
  dynamic value = "hello";
  if (value is String) {
    print("value is string");
  }
}

void testObjectTypeCheck() {
  Object obj = 42;
  if (obj is int) {
    print("obj is int");
  }
}

void testValidTypeCheckWithParameter() {
  void checkType(dynamic param) {
    if (param is String) {
      print("param is string");
    }
  }
}

void testValidListTypeCheck() {
  dynamic data = [1, 2, 3];
  if (data is List) {
    print("data is list");
  }
}

void testValidMapTypeCheck() {
  dynamic config = {"key": "value"};
  if (config is Map) {
    print("config is map");
  }
}

void testValidNullableTypeCheck() {
  Object? value = "text";
  if (value is String) {
    print("value is string");
  }
}

void testValidUnionLikeType() {
  Object data = 42;
  if (data is int || data is String) {
    print("data is int or string");
  }
}
