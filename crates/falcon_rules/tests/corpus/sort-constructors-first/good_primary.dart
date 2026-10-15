// @dart = 3.13

class C() {
  this;
  C.named() : this();
}
enum E(int x) {
  v(1);
  this;
  const E.named() : this(2);
}
