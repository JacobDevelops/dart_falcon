import 'dart:math' as legacy; /* expect: avoid-banned-names */

void example() {
  var legacyValue = 1; /* expect: avoid-banned-names */
  print(legacyValue);
}

class legacyClass { /* expect: avoid-banned-names */
  int legacyField = 0; /* expect: avoid-banned-names */
  void legacyMethod() {} /* expect: avoid-banned-names */
}

void loopsAndCatches(List<int> items) {
  for (final legacyItem in items) { /* expect: avoid-banned-names */
    print(legacyItem);
  }
  try {
    throw 0;
  } catch (legacyError, legacyStack) { /* expect: avoid-banned-names */ /* expect: avoid-banned-names */
    print(legacyError);
  }
}
class Model {
  final int legacyField; /* expect: avoid-banned-names */
  Model(this.legacyField);
  Model.legacyNamed(this.legacyField); /* expect: avoid-banned-names */
}
class Base {
  Base({int? legacyKey}); /* expect: avoid-banned-names */
  void method({int? legacyArg}) {} /* expect: avoid-banned-names */
}
class Child extends Base {
  Child({super.legacyKey});
  @override
  void method({int? legacyArg}) {}
}
