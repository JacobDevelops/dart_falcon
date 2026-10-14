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

typedef DynamicAlias = List<dynamic>; /* expect: avoid-dynamic */

(dynamic, {dynamic value}) recordTypes; /* expect: avoid-dynamic */ /* expect: avoid-dynamic */

class DynamicInitializers {
  final values = <dynamic>[]; /* expect: avoid-dynamic */

  DynamicInitializers() {
    final local = <dynamic>[]; /* expect: avoid-dynamic */
  }

  set value(Object value) {
    dynamic local = value; /* expect: avoid-dynamic */
  }

  Object operator +(Object other) {
    dynamic local = other; /* expect: avoid-dynamic */
    return local;
  }
}

void dynamicSwitch(Object input) {
  switch (input) {
    default:
      dynamic local = input; /* expect: avoid-dynamic */
  }
}

typedef BoundedFunction = void Function<T extends List<dynamic>>(); /* expect: avoid-dynamic */

void dynamicPatterns(Object input) {
  if (input case List<dynamic> values) { /* expect: avoid-dynamic */
    print(values);
  }
  final values = [
    for (List<dynamic> value in <List<Object>>[]) value, /* expect: avoid-dynamic */
  ];
}

class DynamicBase extends Base<List<dynamic>> {} /* expect: avoid-dynamic */

@Annotation<List<dynamic>>() /* expect: avoid-dynamic */
void annotatedDynamic() {}

Object dynamicSwitchExpression(Object input) => switch (input) {
  List<dynamic> values => values, /* expect: avoid-dynamic */
  _ => input,
};
