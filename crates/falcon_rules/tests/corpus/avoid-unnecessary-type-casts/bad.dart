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

// Regression: the cast must still be found inside Dart 3 containers.
void testCastsInContainers() {
  final int n = 5;
  final (a, _) = (n as int, 0); /* expect: avoid-unnecessary-type-casts */
  lbl: {
    final b = n as int; /* expect: avoid-unnecessary-type-casts */
    print(b);
  }
  final c = switch (n) {
    0 => n as int, /* expect: avoid-unnecessary-type-casts */
    _ => 0,
  };
  final d = switch (n as int) { /* expect: avoid-unnecessary-type-casts */
    _ => 0,
  };
  final e = [if (n > 0) n as int]; /* expect: avoid-unnecessary-type-casts */
  final f = [...[n as int]]; /* expect: avoid-unnecessary-type-casts */
  final g = (p: n as int, q: 0); /* expect: avoid-unnecessary-type-casts */
  print([a, c, d, e, f, g]);
}

// Regression: locals declared inside a loop or a try body are tracked, so a
// redundant cast there is still reported.
void loopScopeRegression(List<int> xs) {
  for (final _ in xs) {
    final int inLoop = 1;
    print(inLoop as int); /* expect: avoid-unnecessary-type-casts */
  }
  try {
    final int inTry = 2;
    print(inTry as int); /* expect: avoid-unnecessary-type-casts */
  } catch (e) {
    print(e);
  }
}
