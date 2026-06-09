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

Map<String, dynamic> mapWithDynamic = {}; /* expect: avoid-dynamic */

List<dynamic> listOfDynamic = []; /* expect: avoid-dynamic */
