// Accessors with real logic, lone accessors, or plain public fields.
class A {
  int _value = 0;
  int get value => _value * 2;
  set value(int v) => _value = v;
}

class B {
  int _n = 0;
  int get n => _n;
}

class C {
  int _c = 0;
  set c(int v) {
    _c = v;
  }
}

class D {
  int _d = 0;
  int get d => _d;
  set d(int v) {
    _d = v < 0 ? 0 : v;
  }
}

class E {
  int value = 0;
}

class F {
  int _f = 0;
  int get f => _f;
  set f(int v) {
    _f = v;
    print(v);
  }
}
