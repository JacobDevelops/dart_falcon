import 'dart:math' as modern;
import 'support.dart';

// legacyValue in comments and strings is not an identifier.
void example() {
  var modernValue = 'legacyValue';
  print(modernValue);
  var LegacyValue = 1;
}

class Derived {
  @override
  String toString() => 'Derived';
}

class Model {
  final int modernField;
  Model(this.modernField);
  Model.modernNamed(this.modernField);
}
class Child extends Base {
  Child({super.modernKey});
  @override
  void method({int? legacyArg}) {}
}
void loopsAndCatches(List<int> items, int modernItem) {
  for (final currentItem in items) {}
  for (modernItem in items) {}
  try {
    throw 0;
  } catch (modernError, modernStack) {}
}
