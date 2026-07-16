// Type annotations on initializing formals and super parameters.
class A {
  int x;
  double y;
  A(int this.x, double this.y); /* expect: type-init-formals */ /* expect: type-init-formals */
}

class Base {
  final int a;
  Base(this.a);
}

class Derived extends Base {
  Derived(int super.a); /* expect: type-init-formals */
}

class C {
  String name;
  num size;
  C(String this.name); /* expect: type-init-formals */
  C.sized(num this.size); /* expect: type-init-formals */
}

class D {
  bool flag;
  D(bool this.flag); /* expect: type-init-formals */
}
