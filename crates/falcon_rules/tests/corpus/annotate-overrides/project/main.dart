import 'base.dart' show Base;
import 'duplicate.dart' as duplicate;
import 'operators.dart' as operators;

class Child extends Base {
  void method(int value) {} /* expect: annotate-overrides */
  int get count => 1; /* expect: annotate-overrides */
  set label(String value) {} /* expect: annotate-overrides */
  int field = 1; /* expect: annotate-overrides */
  int get label => 0;
  set readOnly(int value) {}
  set writable(int value) {} /* expect: annotate-overrides */
  final int writeOnly = 0;
  void staticOnly() {}
  void _secret() {}
}

class Unrelated extends duplicate.Base {
  void method(int value) {}
}

class OperatorChild extends operators.Vec {
  @Deprecated('use operator - instead')
  operators.Vec operator +(operators.Vec other) => other; /* expect: annotate-overrides */
}
