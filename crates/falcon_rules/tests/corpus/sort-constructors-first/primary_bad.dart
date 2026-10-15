// @dart = 3.13

enum E(int x) {
  v(1);
  void f() {}
  this; /* expect: sort-constructors-first */
}
extension type ET(int x) {
  void f() {}
  this; /* expect: sort-constructors-first */
}
class C() {
  void f() {}
  this; /* expect: sort-constructors-first */
}
class Asserted(var int x) {
  int get y => x;
  this { /* expect: sort-constructors-first */
    assert(y > 0);
  }
}
