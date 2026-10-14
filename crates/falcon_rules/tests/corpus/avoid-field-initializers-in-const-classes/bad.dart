class A {
  final a = const []; /* expect: avoid-field-initializers-in-const-classes */
  const A();
}

class B {
  final List<int> b;
  const B() : b = const []; /* expect: avoid-field-initializers-in-const-classes */
}

class C {
  final int x = 1, y = 2; /* expect: avoid-field-initializers-in-const-classes */ /* expect: avoid-field-initializers-in-const-classes */
  const C();
}

// Initializer-list entry that ignores every parameter.
class D {
  final int value;
  final String label;
  const D(this.value) : label = 'fixed'; /* expect: avoid-field-initializers-in-const-classes */
}

// A field initializer is flagged even when another constructor is not const.
class E {
  final String name = 'e'; /* expect: avoid-field-initializers-in-const-classes */
  const E();
  E.other();
}

mixin class F {
  final int f = 0; /* expect: avoid-field-initializers-in-const-classes */
  const F();
}
