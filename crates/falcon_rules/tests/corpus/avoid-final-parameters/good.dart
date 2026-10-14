void f(int x) {}

void g(int a, [int b = 0]) {}

void h({String? name}) {}

class A {
  final int value;
  final int other;

  // Initializing formals may be final.
  A(final this.value, this.other);

  void m(int x) {
    final y = x;
    print(y);
  }

  set setter(int v) {}
}

class B extends A {
  // Super parameters may be final.
  B(final super.value, super.other);
}

// Function-typed parameters are not reported.
void callback(final void cb()) {}

void j() {
  final local = 1;
  print(local);
}
