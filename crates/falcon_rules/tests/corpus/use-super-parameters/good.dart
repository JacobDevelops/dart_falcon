// Cases that must not be flagged.
class Base {
  Base(int a);
  Base.two(int a, int b);
}

class A extends Base {
  A(int a) : super(a) {
    print(a);
  }
}

class B extends Base {
  B(super.a);
}

class C extends Base {
  C(int value) : super(value + 1);
}

class D extends Base {
  D(int a, int b) : super.two(b, a);
}

class E extends Base {
  E(int a, int b) : super.two(a, b) {
    print(a);
    print(b);
  }
}

class PartialExpression extends Base {
  PartialExpression(int a, int b) : super.two(a, b + 1);
}
class FieldBlocker extends Base {
  final int field;
  FieldBlocker(this.field, int a) : super(a);
}
class NamedToPositional extends Base {
  NamedToPositional({required int a}) : super(a);
}
class WideBase {
  WideBase({num? value});
}
class NarrowParameter extends WideBase {
  NarrowParameter({int? value}) : super(value: value);
}

class NamedBase {
  NamedBase({int? value});
}

class FinalDeclaringPrimary({final int? value}) extends NamedBase {
  this : super(value: value);
}

class VarDeclaringPrimary({var int? value}) extends NamedBase {
  this : super(value: value);
}

class RepeatedArgument extends Base {
  RepeatedArgument(int a) : super.two(a, a);
}
