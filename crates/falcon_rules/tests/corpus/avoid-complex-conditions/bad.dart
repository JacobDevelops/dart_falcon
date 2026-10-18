void f(bool a, bool b, bool c, bool d) {
  if (a && b && c && d) {} /* expect: avoid-complex-conditions */
  final x = a || b || c || d; /* expect: avoid-complex-conditions */
  final y = (a && b) || (c && d); /* expect: avoid-complex-conditions */
  if (!(a || b || c || d)) {} /* expect: avoid-complex-conditions */
  final z = a ? (b && c && d && a) : false; /* expect: avoid-complex-conditions */
  final callback = a && [1].any((x) => a && b && c && d); /* expect: avoid-complex-conditions */
  while (a && b && c && d) { break; } /* expect: avoid-complex-conditions */
}
