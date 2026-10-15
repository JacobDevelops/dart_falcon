void f() {
  for (var _ in [1]) {} /* expect: prefer-final-in-for-each */
}
