abstract class Base { void method(int value, [String? label]); }
abstract class Child extends Base {
  void method(int renamed, [String? other]); /* expect: avoid-renaming-method-parameters */ /* expect: avoid-renaming-method-parameters */
}

class Equality {
  bool operator ==(Object value) => false; /* expect: avoid-renaming-method-parameters */
  dynamic noSuchMethod(Invocation call) => null; /* expect: avoid-renaming-method-parameters */
}
