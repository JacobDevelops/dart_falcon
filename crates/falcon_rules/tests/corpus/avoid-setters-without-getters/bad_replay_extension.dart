class A { int i = 0; }
extension type E(A a) implements A {
  set i(int value) {} /* expect: avoid-setters-without-getters */
}
