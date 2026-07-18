// Uses initializing formals, or assignments that are not a verbatim copy.
class A {
  int x;
  A(this.x);
}

class B {
  int y;
  B(int value) : y = value;
}

class C {
  int a;
  C(int a) : a = a + 1;
}

class D {
  int v;
  D(this.v);
}

class E {
  int w;
  E(int input) { w = input; }
}

class F {
  int z;
  F(this.z) { print(z); }
}
