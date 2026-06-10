// Test cases for avoid-unrelated-type-assertions rule
// Flags 'is T' checks that are structurally impossible

void testStringIsInt() {
  if ("hello" is int) { /* expect: avoid-unrelated-type-assertions */
    print("string is int");
  }
}

void testIntIsString() {
  if (42 is String) { /* expect: avoid-unrelated-type-assertions */
    print("int is string");
  }
}

void testDoubleIsBool() {
  if (3.14 is bool) { /* expect: avoid-unrelated-type-assertions */
    print("double is bool");
  }
}

void testListIsMap() {
  if ([1, 2, 3] is Map) { /* expect: avoid-unrelated-type-assertions */
    print("list is map");
  }
}

void testMultipleImpossibleAssertions() {
  final x = "test";
  final y = 42;
  final z = true;

  if (x is int) { /* expect: avoid-unrelated-type-assertions */
    print("x is int");
  }

  if (y is String) { /* expect: avoid-unrelated-type-assertions */
    print("y is string");
  }

  if (z is List) { /* expect: avoid-unrelated-type-assertions */
    print("z is list");
  }
}

void testStringIsMapType() {
  if ("value" is Map<String, int>) { /* expect: avoid-unrelated-type-assertions */
    print("string is map");
  }
}

void testConditionalExpressionImpossible() {
  final result = "text" is int ? "yes" : "no"; /* expect: avoid-unrelated-type-assertions */
}
