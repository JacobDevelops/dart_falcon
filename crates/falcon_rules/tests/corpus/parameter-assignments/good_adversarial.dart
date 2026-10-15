// Analyzer-confirmed AdvB3: parameter visitors stop at foreign assignments.
class A {
  int v = 0;
}

void cascade(A a, int x) {
  a..v = (x = 2);
}

// A constructor parameter is checked only inside its body.
class C {
  final Function f;
  C(int x) : f = (() => x = 1);
}
