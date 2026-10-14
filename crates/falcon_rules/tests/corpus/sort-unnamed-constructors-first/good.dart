class A {
  A();
  A.named();
}

class B {
  int x = 0;
  B.new();
  B.a();
}

class C {
  C.named();
}

class D {
  void m() {}
  D();
}

enum E {
  one;

  const E();
  const E.named();
}
