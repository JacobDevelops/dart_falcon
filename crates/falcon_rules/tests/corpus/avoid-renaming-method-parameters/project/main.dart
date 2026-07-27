import 'base.dart' show Base;
import 'duplicate.dart' as duplicate;

class Child extends Base {
  void method(int renamed, [String? other]) {} /* expect: avoid-renaming-method-parameters */ /* expect: avoid-renaming-method-parameters */
  set item(Object renamed) {} /* expect: avoid-renaming-method-parameters */
  void staticMethod(int renamed) {}
  void _private(int renamed) {}
}

class Unrelated extends duplicate.Base {
  void method(int unrelated) {}
}
