// Referencing a wildcard variable or parameter is not allowed.

int a(int _) => _; /* expect: no-wildcard-variable-uses */

int b(int _) => _ * 2; /* expect: no-wildcard-variable-uses */

void c() {
  var _ = 1;
  print(_); /* expect: no-wildcard-variable-uses */
}

void d() {
  var __ = 2;
  print(__); /* expect: no-wildcard-variable-uses */
}

int e(int _) {
  return _ + 1; /* expect: no-wildcard-variable-uses */
}

void f() {
  var _ = 0;
  _ = 5; /* expect: no-wildcard-variable-uses */
}
