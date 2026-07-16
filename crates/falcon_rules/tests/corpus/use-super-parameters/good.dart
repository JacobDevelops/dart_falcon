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
