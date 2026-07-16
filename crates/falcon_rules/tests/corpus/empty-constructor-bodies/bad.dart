class A {
  A() {} /* expect: empty-constructor-bodies */
}

class B {
  final int x;
  B(this.x) {} /* expect: empty-constructor-bodies */
}

class C {
  C.named() {} /* expect: empty-constructor-bodies */
}

class D {
  int y = 0;
  D() : y = 1 {} /* expect: empty-constructor-bodies */
}

class E {
  E.first() {} /* expect: empty-constructor-bodies */
  E.second() {} /* expect: empty-constructor-bodies */
}
