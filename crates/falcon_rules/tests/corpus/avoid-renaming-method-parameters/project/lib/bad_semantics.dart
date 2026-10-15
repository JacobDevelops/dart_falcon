abstract class AbstractBase {
  void abstractOnly(int value);
}

class ConcreteBase {
  void concrete(int value, int other) {}
  bool operator ==(Object other) => false;
}

abstract class FromAbstract extends AbstractBase {
  // Only an abstract member is inherited: no concrete parent.
  @override
  void abstractOnly(int renamed);
}

class FromInterface implements ConcreteBase {
  @override
  void concrete(int renamed, int other) {}
}

class Shorter extends ConcreteBase {
  @override
  void concrete(int renamed) {} /* expect: avoid-renaming-method-parameters */
}

class Documented extends ConcreteBase {
  /// Documented overrides are exempt.
  @override
  void concrete(int a, int b) {}
}

class _Private extends ConcreteBase {
  @override
  void concrete(int a, int b) {}
}

mixin OnBase on AbstractBase {
  @override
  void abstractOnly(int renamed) {} /* expect: avoid-renaming-method-parameters */
}
