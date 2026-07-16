// Initializing formals and super parameters without a redundant type.
class A {
  int x;
  double y;
  A(this.x, this.y);
}

class Base {
  final int a;
  Base(this.a);
}

class Derived extends Base {
  Derived(super.a);
}

class C {
  String name;
  num size;
  C(this.name);
  C.sized(this.size);
  void use() {
    print(this.name);
    return this.size.isNaN ? null : null;
  }
}

class D {
  bool flag;
  D(this.flag);
}
