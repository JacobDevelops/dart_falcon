// Locals without leading underscores; wildcards and members are exempt.

int _private = 0;

class C {
  int _field = 0;
}

void f(int param) {
  print(param);
}

void g() {
  var local = 1;
  print(local);
}

void h(int _) {}

void i() {
  var __ = 1;
  print(__);
}
