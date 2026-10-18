void f(bool a, bool b, bool c) {
  while (a && b && c) { break; } /* expect: avoid-complex-loop-conditions */
  while (a || b || c) { break; } /* expect: avoid-complex-loop-conditions */
  do {} while (a && (b || c)); /* expect: avoid-complex-loop-conditions */
  for (; a && b && c;) { break; } /* expect: avoid-complex-loop-conditions */
  for (var i = 0; i < 4 && a && b; i++) {} /* expect: avoid-complex-loop-conditions */
  final values = [for (var i = 0; i < 4 && a && b; i++) i]; /* expect: avoid-complex-loop-conditions */
}
