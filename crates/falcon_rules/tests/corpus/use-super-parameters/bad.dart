// Parameters forwarded verbatim to super and used nowhere else.
class Base {
  Base(int a);
  Base.two(int a, int b);
  Base.named({int? x});
}

class A extends Base {
  A(int a) : super(a); /* expect: use-super-parameters */
}

class B extends Base {
  B(int a, int b) : super.two(a, b); /* expect: use-super-parameters */ /* expect: use-super-parameters */
}

class C extends Base {
  C({int? x}) : super.named(x: x); /* expect: use-super-parameters */
}

class D extends Base {
  D(int a) : super(a); /* expect: use-super-parameters */
}

class E extends Base {
  E(int a) : super(a); /* expect: use-super-parameters */
}

class ObjectBase {
  ObjectBase(Object e);
}

class CatchBinding extends ObjectBase {
  CatchBinding(Object e) : super(e) { /* expect: use-super-parameters */
    try {} catch (e) {}
  }
}
