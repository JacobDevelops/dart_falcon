// Uses initializing formals, or assignments that cannot become one.
class A {
  int x;
  A(this.x);
}

class C {
  int a;
  C(int a) : a = a + 1; // not a verbatim copy
}

class D {
  int v;
  D(this.v);
}

class G {
  int p, q;
  G(int v) : p = v, q = v; // one parameter, two fields — no initializing formal
}

class H {
  int r;
  H({required int value}) : r = value; // renaming a named parameter breaks callers
}

class F {
  int z;
  F(this.z) { print(z); }
}

class DifferentName {
  int value;
  DifferentName(int input) : value = input;
}

class PrivatePositional {
  int _value;
  PrivatePositional(int value) : _value = value;
}

class Parent {
  int value = 0;
}
class Inherited extends Parent {
  Inherited(int value) { this.value = value; }
}
class Setter {
  set value(int value) {}
  Setter(int value) { this.value = value; }
}
class Reused {
  int value;
  Reused(int value) : value = value { print(value); }
}
class DynamicCopy {
  int value;
  DynamicCopy(dynamic value) : value = value;
}

class StaticField {
  static int value = 0;
  StaticField(int value) { StaticField.value = value; }
}
class ParenthesizedInitializer {
  int value;
  ParenthesizedInitializer(int value) : value = (value);
}

class InitializerReuse {
  int value;
  int other;
  InitializerReuse(int value) : value = value, other = value;
}

class ShadowedParameter {
  late int value;
  ShadowedParameter(int value) : assert(value > 0) {
    final value = 0;
    this.value = value;
  }
}

class DeclaringPrimary(var int value) {
  int other;
  this : other = value {
    this.value = value;
  }
}
