class Base { void method() {} }
class Child extends Base {
  @override
  void method() {}
  static void helper() {}
}

class SetterOnlyBase { set onlySetter(int value) {} }
class FieldOverSetter implements SetterOnlyBase {
  // Fields are compared through their getter; only a setter is inherited.
  int onlySetter = 0;
}

class UnaryOnly { UnaryOnly operator -() => this; }
class BinaryMinus extends UnaryOnly {
  UnaryOnly operator -(UnaryOnly other) => this;
}

extension type Wrapper(int value) implements Object {
  String toString() => '';
}
