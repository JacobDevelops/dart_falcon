void f() {
  return;
}

int g() {
  return null;
}

int? h() => null;

Future<void> i() async {
  await Future<void>.value();
}

class C {
  int _v = 0;

  void method() {
    print('no return');
  }

  int compute() => null;

  set value(int v) {
    _v = v;
  }
}

void outer() {
  int inner() {
    return null;
  }

  inner();
}
