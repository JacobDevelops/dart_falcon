void f1(StringBuffer a) {
  a..write('x'); /* expect: avoid-single-cascade-in-expression-statements */
}

void f2(List<int> list) {
  list..add(1); /* expect: avoid-single-cascade-in-expression-statements */
}

void f3(Foo b) {
  b..field = 1; /* expect: avoid-single-cascade-in-expression-statements */
}

void f4(List<int> c) {
  c..[0] = 2; /* expect: avoid-single-cascade-in-expression-statements */
}

void f5(Foo d) {
  d..method(); /* expect: avoid-single-cascade-in-expression-statements */
}

void f6(StringBuffer e) {
  e..clear(); /* expect: avoid-single-cascade-in-expression-statements */
}

// Closure argument of a constructor invocation (`Expr::New`).
Widget f7(StringBuffer a) {
  return new ElevatedButton(onPressed: () {
    a..write('x'); /* expect: avoid-single-cascade-in-expression-statements */
  });
}

// Closure nested inside a collection literal.
List<VoidCallback> f8(StringBuffer a) {
  return [
    () {
      a..clear(); /* expect: avoid-single-cascade-in-expression-statements */
    },
  ];
}
