// @dart = 3.12

void f(final int x) {} /* expect: avoid-final-parameters */

void g(int a, [final int b = 0]) {} /* expect: avoid-final-parameters */

void h({final String? name}) {} /* expect: avoid-final-parameters */

void i({required final String id}) {} /* expect: avoid-final-parameters */

class A {
  A(final int value); /* expect: avoid-final-parameters */

  void m(final x) {} /* expect: avoid-final-parameters */

  set value(final int v) {} /* expect: avoid-final-parameters */

  int operator +(final int other) => other; /* expect: avoid-final-parameters */
}

void j() {
  void local(final int x) {} /* expect: avoid-final-parameters */
  final literal = (final int y) => y; /* expect: avoid-final-parameters */
  local(1);
  literal(2);
}
