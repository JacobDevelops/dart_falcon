// Test cases for avoid-unnecessary-type-casts rule
// Flags 'as T' casts where the variable is already T

void testCastExplicitIntType() {
  final int x = 5;
  final y = x as int; /* expect: avoid-unnecessary-type-casts */
}

void testCastExplicitStringType() {
  final String name = "hello";
  final result = name as String; /* expect: avoid-unnecessary-type-casts */
}

void testCastExplicitListType() {
  final List<String> items = [];
  final casted = items as List<String>; /* expect: avoid-unnecessary-type-casts */
}

void testMultipleUnnecessaryCasts() {
  final bool active = true;
  final double value = 3.14;

  final a = active as bool; /* expect: avoid-unnecessary-type-casts */
  final b = value as double; /* expect: avoid-unnecessary-type-casts */
}

class MyClass {
  final int id = 42;

  void castId() {
    final casted = id as int; /* expect: avoid-unnecessary-type-casts */
    print(casted);
  }
}

void testInlineUnnecessaryCast() {
  final String message = "test";
  final length = (message as String).length; /* expect: avoid-unnecessary-type-casts */
}

void testChainedUnnecessaryCast() {
  final Map<String, int> data = {};
  final result = data as Map<String, int>; /* expect: avoid-unnecessary-type-casts */
}
