// Test cases for avoid-unnecessary-type-assertions rule
// Flags 'is T' checks where the variable is already known to be T

void testExplicitIntType() {
  final int x = 5;
  if (x is int) { /* expect: avoid-unnecessary-type-assertions */
    print("x is int");
  }
}

void testExplicitStringType() {
  final String name = "hello";
  if (name is String) { /* expect: avoid-unnecessary-type-assertions */
    print("name is string");
  }
}

void testExplicitListType() {
  final List<String> items = [];
  if (items is List) { /* expect: avoid-unnecessary-type-assertions */
    print("items is list");
  }
}

void testMultipleUnnecessaryAssertions() {
  final bool active = true;
  final double value = 3.14;

  if (active is bool) { /* expect: avoid-unnecessary-type-assertions */
    print("active is bool");
  }

  if (value is double) { /* expect: avoid-unnecessary-type-assertions */
    print("value is double");
  }
}

class MyClass {
  final int id = 42;

  void checkId() {
    if (id is int) { /* expect: avoid-unnecessary-type-assertions */
      print("id is int");
    }
  }
}

void testInlineAssertion() {
  final String message = "test";
  final result = message is String ? "yes" : "no"; /* expect: avoid-unnecessary-type-assertions */
}
