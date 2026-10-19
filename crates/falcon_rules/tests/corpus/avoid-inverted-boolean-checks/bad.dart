void f(int x, int y, bool a, bool b) {
  if (!(x == y)) {} /* expect: avoid-inverted-boolean-checks */
  if (!(x != y)) {} /* expect: avoid-inverted-boolean-checks */
  if (!(a && b)) {} /* expect: avoid-inverted-boolean-checks */
  while (!(a || b)) {} /* expect: avoid-inverted-boolean-checks */
}
