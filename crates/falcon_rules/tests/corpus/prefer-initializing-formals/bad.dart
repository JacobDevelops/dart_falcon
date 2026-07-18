// Parameters copied verbatim into same-named fields.
class A {
  int x;
  A(int x) : x = x; /* expect: prefer-initializing-formals */
}

class B {
  int y;
  B(int y) { this.y = y; } /* expect: prefer-initializing-formals */
}

class C {
  int a, b;
  C(int a, int b) : a = a, b = b; /* expect: prefer-initializing-formals */ /* expect: prefer-initializing-formals */
}

class D {
  String name;
  D(String name) : name = name; /* expect: prefer-initializing-formals */
}

class E {
  int v;
  E(int v) { this.v = v; } /* expect: prefer-initializing-formals */
}

class F {
  int y;
  F(int value) : y = value; /* expect: prefer-initializing-formals */
}

class G {
  int z;
  G(int input) { this.z = input; } /* expect: prefer-initializing-formals */
}
