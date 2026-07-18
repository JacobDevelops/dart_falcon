// Private fields only ever initialized, never reassigned.
class A {
  int _a = 1; /* expect: prefer-final-fields */
  int get a => _a;
}

class B {
  static int _shared = 0; /* expect: prefer-final-fields */
  static int read() => _shared;
}

class C {
  int _c; /* expect: prefer-final-fields */
  C(this._c);
}

class D {
  int _d; /* expect: prefer-final-fields */
  D(int v) : _d = v;
}

class E {
  String _e = 'x'; /* expect: prefer-final-fields */
  String describe() => _e;
}

class F {
  final List<int> items = [];
  int _f; /* expect: prefer-final-fields */
  F(this._f);
}
