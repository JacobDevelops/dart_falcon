// Fields that are reassigned, already final, public, or late.
class A {
  int _a = 0;
  void inc() {
    _a += 1;
  }
}

class B {
  int _b = 0;
  void assign(int v) {
    _b = v;
  }
}

class C {
  final int _c;
  C(this._c);
}

class D {
  int value = 0;
  void bump() {
    value++;
  }
}

class E {
  late int _e;
  void init() {
    _e = 5;
  }
}

class F {
  int _f = 0;
  void tick() {
    _f++;
  }
}
