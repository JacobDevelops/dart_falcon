class A {
  A();
  int x = 0;
  A.named(); /* expect: sort-constructors-first */
  factory A.make() => A(); /* expect: sort-constructors-first */
}

class B {
  void m() {}
  B(); /* expect: sort-constructors-first */
}

enum E {
  one, two;

  int get value => index;
  const E(); /* expect: sort-constructors-first */
}

abstract class M {
  static const k = 1;
  M(); /* expect: sort-constructors-first */
}

extension type Id(int value) {
  int get doubled => value * 2;
  Id.zero() : value = 0; /* expect: sort-constructors-first */
}
