class A {
  A.named();
  A(); /* expect: sort-unnamed-constructors-first */
}

class B {
  B.a();
  int x = 0;
  B.new(); /* expect: sort-unnamed-constructors-first */
}

class C {
  factory C.make() => C._();
  C._();
  C(); /* expect: sort-unnamed-constructors-first */
}

enum E {
  one;

  const E.named();
  const E(); /* expect: sort-unnamed-constructors-first */
}

extension type Id._(int value) {
  Id.zero() : value = 0;
  Id(this.value); /* expect: sort-unnamed-constructors-first */
}
