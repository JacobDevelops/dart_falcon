class A {
  A();
}

class B {
  final int x;
  B(this.x);
}

class C {
  C.named();
}

class D {
  int y = 0;
  D() : y = 1;
}

class E {
  E() {
    print('init');
  }
}

class F {
  final int z;
  F(this.z) {
    print(z);
  }
}
