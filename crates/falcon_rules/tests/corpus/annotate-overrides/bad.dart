class Base {
  void method(int value) {}
  int get count => 0;
  int field = 0;
}
class Child extends Base {
  void method(int value) {} /* expect: annotate-overrides */
  int get count => 1; /* expect: annotate-overrides */
  int field = 1; /* expect: annotate-overrides */
}

class SetterOnly { set onlySetter(int value) {} }
class SetterOverride extends SetterOnly {
  set onlySetter(int value) {} /* expect: annotate-overrides */
}

class Negatable {
  Negatable operator -() => this;
  Negatable operator +(Negatable other) => this;
}
class BinaryOverride extends Negatable {
  Negatable operator +(Negatable other) => this; /* expect: annotate-overrides */
}
