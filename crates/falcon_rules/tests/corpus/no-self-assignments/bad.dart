class C {
  int x = 0;
  int y = 0;

  C(int x, int y) {
    x = x; /* expect: no-self-assignments */
    y = y; /* expect: no-self-assignments */
  }

  void update(int x) {
    x = x; /* expect: no-self-assignments */
  }

  void field() {
    x = x; /* expect: no-self-assignments */
  }
}

class Holder {
  int value = 0;
}

void prefixed(Holder holder) {
  holder.value = holder.value; /* expect: no-self-assignments */
}

int top = 0;

void topLevel() {
  top = top; /* expect: no-self-assignments */
}

void inExpression(int a) {
  print(a = a); /* expect: no-self-assignments */
}

void inClosure() {
  var local = 1;
  void f() {
    local = local; /* expect: no-self-assignments */
  }

  f();
}

void comments(int value) {
  value /* left */ = /* right */ value; /* expect: no-self-assignments */
  value = /* expect: no-self-assignments */ // Comment before the RHS.
      value;
}
