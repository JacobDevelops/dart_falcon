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
