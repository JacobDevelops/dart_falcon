class A {
  String? p;
  A(this.p) : assert(p != null); /* expect: tighten-type-of-initializing-formals */
  A.reversed(this.p) : assert(null != p); /* expect: tighten-type-of-initializing-formals */
  A.typed(String? this.p) : assert(p != null); /* expect: tighten-type-of-initializing-formals */
  A.named({this.p}) : assert(p != null); /* expect: tighten-type-of-initializing-formals */
}

class B {
  int? x;
  int? y;
  B(
    this.x, /* expect: tighten-type-of-initializing-formals */
    this.y, /* expect: tighten-type-of-initializing-formals */
  ) : assert(x != null),
      assert(y != null);
}

class Base {
  final String? name;
  Base(this.name);
}

class Derived extends Base {
  Derived(super.name) : assert(name != null); /* expect: tighten-type-of-initializing-formals */
}

class Untyped {
  var value;
  Untyped(this.value) : assert(value != null); /* expect: tighten-type-of-initializing-formals */
}

class UnresolvedNullable {
  Missing? value;
  UnresolvedNullable(this.value) : assert(value != null); /* expect: tighten-type-of-initializing-formals */
}
