class A {
  String? p;
  A.typed(String this.p);
  A.other(this.p);
  A.unrelated(this.p, String? q) : assert(q != null);
  A.nonNull(String this.p) : assert(p != null);
  A.otherCheck(this.p) : assert(p == null);
}

class B {
  final int x;
  B(this.x) : assert(x != null);
}

class C {
  final String? s;
  C(String? value) : s = value, assert(value != null);
}

class Base {
  final String name;
  Base(this.name);
}

class Derived extends Base {
  Derived(super.name) : assert(name != null);
}

class Inferred {
  var value = 0;
  Inferred(this.value) : assert(value != null);
}
