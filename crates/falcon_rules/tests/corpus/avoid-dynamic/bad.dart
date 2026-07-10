// Test cases for avoid-dynamic rule
// All violations are annotated inline

void testDynamicVariable() {
  dynamic x = 5; /* expect: avoid-dynamic */
  dynamic name = "test"; /* expect: avoid-dynamic */
  dynamic list = [1, 2, 3]; /* expect: avoid-dynamic */
}

void testDynamicParameters(dynamic arg, dynamic other) { /* expect: avoid-dynamic */ /* expect: avoid-dynamic */
  print(arg);
  print(other);
}

Future<dynamic> asyncMethod() { /* expect: avoid-dynamic */
  return Future.value(42);
}

dynamic returnDynamic() { /* expect: avoid-dynamic */
  return "anything";
}

class TestClass {
  dynamic field; /* expect: avoid-dynamic */

  void method(dynamic param) { /* expect: avoid-dynamic */
    final dynamic local = param; /* expect: avoid-dynamic */
  }
}

// `dynamic` as a type argument of a non-Map generic is still flagged, and a
// `dynamic` nested one level below `Map` (inside `List`) is NOT exempt: only a
// direct `Map<_, dynamic>` argument is.
List<dynamic> listOfDynamic = []; /* expect: avoid-dynamic */

Route<dynamic>? routeField; /* expect: avoid-dynamic */

Map<String, List<dynamic>> nested = {}; /* expect: avoid-dynamic */
