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
