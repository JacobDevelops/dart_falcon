// A getter instead of a field.
class A {
  List get a => const [];
  const A();
}

// Initializer-list entry that uses a constructor parameter.
class B {
  final int b;
  const B(int value) : b = value * 2;
}

// No const constructor: field initializers are fine.
class C {
  final c = const [];
  C();
}

// Static fields are not per-instance.
class D {
  static final d = <int>[];
  static const e = 1;
  const D();
}

// Several constructors: the initializer list is not checked.
class E {
  final int e;
  const E() : e = 0;
  const E.named(this.e);
}

// Implicit default constructor is never const.
class F {
  final f = 1;
}

// Field formal parameters, no initializers.
class G {
  final int x;
  final int y;
  const G(this.x, {this.y = 0});
}

// Parameter referenced in a nested expression.
class H {
  final int next;
  const H(int first) : next = (first + 1) * 2;
}
